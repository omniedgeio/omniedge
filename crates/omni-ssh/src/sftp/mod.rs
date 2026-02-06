//! SFTP (SSH File Transfer Protocol) implementation
//!
//! This module provides SFTP server and client functionality for secure
//! file transfer between OmniEdge peers.
//!
//! ## Features
//!
//! - **Server**: Implements `russh_sftp::server::Handler` for handling SFTP requests
//! - **Client**: Wrapper around `russh_sftp::client::SftpSession` for easy file transfers
//! - **Path restrictions**: Integrates with policy to restrict accessible paths
//! - **Read-only mode**: Support for read-only SFTP access

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

/// SFTP server configuration
#[derive(Debug, Clone)]
pub struct SftpServerConfig {
    /// Root directory for SFTP access (chroot-like behavior)
    pub root_dir: Option<PathBuf>,
    /// Whether to allow read operations
    pub allow_read: bool,
    /// Whether to allow write operations
    pub allow_write: bool,
    /// Maximum file size for transfers (bytes)
    pub max_file_size: Option<u64>,
    /// Allowed paths (if set, only these paths are accessible)
    pub allowed_paths: Option<Vec<String>>,
}

impl Default for SftpServerConfig {
    fn default() -> Self {
        Self {
            root_dir: None,
            allow_read: true,
            allow_write: true,
            max_file_size: None,
            allowed_paths: None,
        }
    }
}

impl SftpServerConfig {
    /// Create a read-only configuration
    pub fn read_only(root_dir: Option<PathBuf>) -> Self {
        Self {
            root_dir,
            allow_read: true,
            allow_write: false,
            max_file_size: None,
            allowed_paths: None,
        }
    }

    /// Create a configuration from SSH action
    pub fn from_action(action: &crate::types::SshAction) -> Self {
        Self {
            root_dir: None, // Will be set to user's home directory
            allow_read: true,
            allow_write: !action.read_only,
            max_file_size: None,
            allowed_paths: action.allowed_paths.clone(),
        }
    }
}

/// File information
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// File name
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// Whether it's a directory
    pub is_dir: bool,
    /// Last modified timestamp (Unix epoch)
    pub modified: u64,
    /// File permissions (Unix mode)
    pub permissions: Option<u32>,
}

// SFTP Server implementation (feature-gated)
#[cfg(feature = "sftp")]
mod server_impl {
    use super::*;
    use russh_sftp::protocol::{
        Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Packet, Status, StatusCode,
        Version,
    };
    use std::io::SeekFrom;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::fs::{self, File as TokioFile, OpenOptions};
    use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
    use tokio::sync::Mutex;

    /// Error type for SFTP operations
    #[derive(Debug, thiserror::Error)]
    pub enum SftpError {
        #[error("Permission denied: {0}")]
        PermissionDenied(String),
        #[error("No such file or directory: {0}")]
        NoSuchFile(String),
        #[error("File exists: {0}")]
        FileExists(String),
        #[error("Not a directory: {0}")]
        NotDirectory(String),
        #[error("Is a directory: {0}")]
        IsDirectory(String),
        #[error("Invalid handle: {0}")]
        InvalidHandle(String),
        #[error("Operation not supported")]
        NotSupported,
        #[error("I/O error: {0}")]
        Io(#[from] std::io::Error),
        #[error("Path escapes root directory")]
        PathEscape,
        #[error("Read-only mode")]
        ReadOnly,
        #[error("File too large")]
        FileTooLarge,
        #[error("End of file")]
        Eof,
    }

    impl From<SftpError> for StatusCode {
        fn from(err: SftpError) -> Self {
            match err {
                SftpError::PermissionDenied(_) => StatusCode::PermissionDenied,
                SftpError::NoSuchFile(_) => StatusCode::NoSuchFile,
                SftpError::FileExists(_) => StatusCode::Failure,
                SftpError::NotDirectory(_) => StatusCode::Failure,
                SftpError::IsDirectory(_) => StatusCode::Failure,
                SftpError::InvalidHandle(_) => StatusCode::Failure,
                SftpError::NotSupported => StatusCode::OpUnsupported,
                SftpError::Io(ref e) => match e.kind() {
                    std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
                    std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
                    std::io::ErrorKind::AlreadyExists => StatusCode::Failure,
                    _ => StatusCode::Failure,
                },
                SftpError::PathEscape => StatusCode::PermissionDenied,
                SftpError::ReadOnly => StatusCode::PermissionDenied,
                SftpError::FileTooLarge => StatusCode::Failure,
                SftpError::Eof => StatusCode::Eof,
            }
        }
    }

    /// Handle for an open file
    struct FileHandle {
        file: TokioFile,
        path: PathBuf,
        write_mode: bool,
    }

    /// Handle for an open directory
    struct DirHandle {
        path: PathBuf,
        entries: Vec<std::fs::DirEntry>,
        position: usize,
        read_complete: bool,
    }

    /// SFTP server handler implementing russh_sftp::server::Handler
    pub struct SftpHandler {
        config: SftpServerConfig,
        /// Next handle ID
        next_handle: AtomicU64,
        /// Open file handles (handle_id -> FileHandle)
        file_handles: Mutex<HashMap<String, FileHandle>>,
        /// Open directory handles (handle_id -> DirHandle)
        dir_handles: Mutex<HashMap<String, DirHandle>>,
        /// Connection ID for logging
        conn_id: String,
        /// Username for logging
        #[allow(dead_code)]
        username: String,
    }

    impl SftpHandler {
        /// Create a new SFTP handler
        pub fn new(config: SftpServerConfig, conn_id: String, username: String) -> Self {
            Self {
                config,
                next_handle: AtomicU64::new(1),
                file_handles: Mutex::new(HashMap::new()),
                dir_handles: Mutex::new(HashMap::new()),
                conn_id,
                username,
            }
        }

        /// Generate a new handle ID
        fn next_handle_id(&self) -> String {
            let id = self.next_handle.fetch_add(1, Ordering::SeqCst);
            format!("h{}", id)
        }

        /// Resolve and validate a path
        fn resolve_path(&self, path_str: &str) -> Result<PathBuf, SftpError> {
            let path = PathBuf::from(path_str);

            // Determine the resolved path
            let resolved = if let Some(ref root) = self.config.root_dir {
                if path.is_absolute() {
                    // Strip leading "/" and join with root
                    let stripped = path.strip_prefix("/").unwrap_or(&path);
                    root.join(stripped)
                } else {
                    root.join(&path)
                }
            } else {
                // No root restriction - use path as-is
                if path.is_absolute() {
                    path
                } else {
                    // Relative to current directory
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(&path)
                }
            };

            // Canonicalize to resolve .. and symlinks
            // Note: The path might not exist yet (for create operations)
            let canonical = if resolved.exists() {
                resolved.canonicalize().map_err(SftpError::Io)?
            } else {
                // For non-existent paths, check the parent
                if let Some(parent) = resolved.parent() {
                    if parent.exists() {
                        let canonical_parent = parent.canonicalize().map_err(SftpError::Io)?;
                        canonical_parent.join(resolved.file_name().unwrap_or_default())
                    } else {
                        resolved
                    }
                } else {
                    resolved
                }
            };

            // Check if path is within root (if root is set)
            if let Some(ref root) = self.config.root_dir {
                let canonical_root = root.canonicalize().unwrap_or_else(|_| root.clone());
                if !canonical.starts_with(&canonical_root) {
                    warn!(
                        "Path escape attempt: {} -> {} (root: {})",
                        path_str,
                        canonical.display(),
                        canonical_root.display()
                    );
                    return Err(SftpError::PathEscape);
                }
            }

            // Check allowed paths
            if let Some(ref allowed) = self.config.allowed_paths {
                let path_str = canonical.to_string_lossy();
                let is_allowed = allowed.iter().any(|p| {
                    if p.ends_with("/*") {
                        let prefix = &p[..p.len() - 2];
                        path_str.starts_with(prefix)
                    } else if p.ends_with("/**") {
                        let prefix = &p[..p.len() - 3];
                        path_str.starts_with(prefix)
                    } else {
                        path_str == p.as_str() || path_str.starts_with(&format!("{}/", p))
                    }
                });

                if !is_allowed {
                    warn!(
                        "Path not in allowed list: {} (allowed: {:?})",
                        path_str, allowed
                    );
                    return Err(SftpError::PermissionDenied(format!(
                        "Path not allowed: {}",
                        path_str
                    )));
                }
            }

            Ok(canonical)
        }

        /// Check if write operations are allowed
        fn check_write(&self) -> Result<(), SftpError> {
            if !self.config.allow_write {
                return Err(SftpError::ReadOnly);
            }
            Ok(())
        }

        /// Check if read operations are allowed
        fn check_read(&self) -> Result<(), SftpError> {
            if !self.config.allow_read {
                return Err(SftpError::PermissionDenied(
                    "Read operations not allowed".to_string(),
                ));
            }
            Ok(())
        }

        /// Convert std::fs::Metadata to FileAttributes
        fn metadata_to_attrs(metadata: &std::fs::Metadata) -> FileAttributes {
            let mut attrs = FileAttributes::default();
            attrs.size = Some(metadata.len());

            // Set file type via permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                attrs.permissions = Some(metadata.mode());
                attrs.uid = Some(metadata.uid());
                attrs.gid = Some(metadata.gid());
            }

            #[cfg(not(unix))]
            {
                // On Windows, set basic permissions
                if metadata.is_dir() {
                    attrs.permissions = Some(0o755 | 0o40000); // Directory
                    attrs.set_dir(true);
                } else if metadata.is_file() {
                    attrs.permissions = Some(0o644 | 0o100000); // Regular file
                    attrs.set_regular(true);
                } else {
                    attrs.permissions = Some(0o644);
                }
            }

            // Set times
            if let Ok(mtime) = metadata.modified() {
                if let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH) {
                    attrs.mtime = Some(duration.as_secs() as u32);
                }
            }
            if let Ok(atime) = metadata.accessed() {
                if let Ok(duration) = atime.duration_since(std::time::UNIX_EPOCH) {
                    attrs.atime = Some(duration.as_secs() as u32);
                }
            }

            attrs
        }

        /// Create a Status with OK
        fn status_ok(id: u32) -> Status {
            Status {
                id,
                status_code: StatusCode::Ok,
                error_message: String::new(),
                language_tag: String::new(),
            }
        }
    }

    impl russh_sftp::server::Handler for SftpHandler {
        type Error = SftpError;

        fn unimplemented(&self) -> Self::Error {
            SftpError::NotSupported
        }

        async fn init(
            &mut self,
            version: u32,
            extensions: HashMap<String, String>,
        ) -> Result<Version, Self::Error> {
            info!(
                "[{}] SFTP init: version={}, extensions={:?}",
                self.conn_id, version, extensions
            );
            Ok(Version::new())
        }

        async fn open(
            &mut self,
            id: u32,
            filename: String,
            pflags: OpenFlags,
            _attrs: FileAttributes,
        ) -> Result<Handle, Self::Error> {
            debug!(
                "[{}] SFTP open: id={}, filename={}, pflags={:?}",
                self.conn_id, id, filename, pflags
            );

            let path = self.resolve_path(&filename)?;

            // Check permissions
            let is_write = pflags.contains(OpenFlags::WRITE)
                || pflags.contains(OpenFlags::CREATE)
                || pflags.contains(OpenFlags::TRUNCATE)
                || pflags.contains(OpenFlags::APPEND);

            if is_write {
                self.check_write()?;
            }
            if pflags.contains(OpenFlags::READ) {
                self.check_read()?;
            }

            // Check max file size for writes
            if is_write {
                if let Some(max_size) = self.config.max_file_size {
                    if path.exists() {
                        let metadata = fs::metadata(&path).await?;
                        if metadata.len() > max_size {
                            return Err(SftpError::FileTooLarge);
                        }
                    }
                }
            }

            // Build open options
            let mut options = OpenOptions::new();

            if pflags.contains(OpenFlags::READ) {
                options.read(true);
            }
            if pflags.contains(OpenFlags::WRITE) {
                options.write(true);
            }
            if pflags.contains(OpenFlags::CREATE) {
                options.create(true);
            }
            if pflags.contains(OpenFlags::TRUNCATE) {
                options.truncate(true);
            }
            if pflags.contains(OpenFlags::APPEND) {
                options.append(true);
            }
            if pflags.contains(OpenFlags::EXCLUDE) {
                options.create_new(true);
            }

            let file = options.open(&path).await.map_err(|e| {
                warn!("[{}] Failed to open file {}: {}", self.conn_id, filename, e);
                SftpError::Io(e)
            })?;

            let handle_id = self.next_handle_id();

            let mut handles = self.file_handles.lock().await;
            handles.insert(
                handle_id.clone(),
                FileHandle {
                    file,
                    path,
                    write_mode: is_write,
                },
            );

            info!(
                "[{}] Opened file: {} -> handle {}",
                self.conn_id, filename, handle_id
            );

            Ok(Handle {
                id,
                handle: handle_id,
            })
        }

        async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
            debug!(
                "[{}] SFTP close: id={}, handle={}",
                self.conn_id, id, handle
            );

            // Try file handles first
            {
                let mut handles = self.file_handles.lock().await;
                if let Some(fh) = handles.remove(&handle) {
                    // File is automatically closed when dropped
                    info!(
                        "[{}] Closed file handle {}: {}",
                        self.conn_id,
                        handle,
                        fh.path.display()
                    );
                    return Ok(Self::status_ok(id));
                }
            }

            // Try directory handles
            {
                let mut handles = self.dir_handles.lock().await;
                if handles.remove(&handle).is_some() {
                    info!("[{}] Closed directory handle {}", self.conn_id, handle);
                    return Ok(Self::status_ok(id));
                }
            }

            Err(SftpError::InvalidHandle(handle))
        }

        async fn read(
            &mut self,
            id: u32,
            handle: String,
            offset: u64,
            len: u32,
        ) -> Result<Data, Self::Error> {
            debug!(
                "[{}] SFTP read: id={}, handle={}, offset={}, len={}",
                self.conn_id, id, handle, offset, len
            );

            self.check_read()?;

            let mut handles = self.file_handles.lock().await;
            let fh = handles
                .get_mut(&handle)
                .ok_or_else(|| SftpError::InvalidHandle(handle.clone()))?;

            fh.file.seek(SeekFrom::Start(offset)).await?;

            let mut buffer = vec![0u8; len as usize];
            let bytes_read = fh.file.read(&mut buffer).await?;

            if bytes_read == 0 {
                return Err(SftpError::Eof);
            }

            buffer.truncate(bytes_read);
            Ok(Data { id, data: buffer })
        }

        async fn write(
            &mut self,
            id: u32,
            handle: String,
            offset: u64,
            data: Vec<u8>,
        ) -> Result<Status, Self::Error> {
            debug!(
                "[{}] SFTP write: id={}, handle={}, offset={}, len={}",
                self.conn_id,
                id,
                handle,
                offset,
                data.len()
            );

            self.check_write()?;

            let mut handles = self.file_handles.lock().await;
            let fh = handles
                .get_mut(&handle)
                .ok_or_else(|| SftpError::InvalidHandle(handle.clone()))?;

            if !fh.write_mode {
                return Err(SftpError::PermissionDenied(
                    "File not opened for writing".to_string(),
                ));
            }

            // Check max file size
            if let Some(max_size) = self.config.max_file_size {
                let new_size = offset + data.len() as u64;
                if new_size > max_size {
                    return Err(SftpError::FileTooLarge);
                }
            }

            fh.file.seek(SeekFrom::Start(offset)).await?;
            fh.file.write_all(&data).await?;

            Ok(Self::status_ok(id))
        }

        async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
            debug!("[{}] SFTP lstat: id={}, path={}", self.conn_id, id, path);

            self.check_read()?;

            let resolved = self.resolve_path(&path)?;
            let metadata = fs::symlink_metadata(&resolved).await?;
            let attrs = Self::metadata_to_attrs(&metadata);

            Ok(Attrs { id, attrs })
        }

        async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
            debug!(
                "[{}] SFTP fstat: id={}, handle={}",
                self.conn_id, id, handle
            );

            self.check_read()?;

            let handles = self.file_handles.lock().await;
            let fh = handles
                .get(&handle)
                .ok_or_else(|| SftpError::InvalidHandle(handle.clone()))?;

            let metadata = fs::metadata(&fh.path).await?;
            let attrs = Self::metadata_to_attrs(&metadata);

            Ok(Attrs { id, attrs })
        }

        async fn setstat(
            &mut self,
            id: u32,
            path: String,
            attrs: FileAttributes,
        ) -> Result<Status, Self::Error> {
            debug!(
                "[{}] SFTP setstat: id={}, path={}, attrs={:?}",
                self.conn_id, id, path, attrs
            );

            self.check_write()?;

            let resolved = self.resolve_path(&path)?;

            // Set permissions if specified
            #[cfg(unix)]
            if let Some(perm) = attrs.permissions {
                use std::os::unix::fs::PermissionsExt;
                let permissions = std::fs::Permissions::from_mode(perm);
                fs::set_permissions(&resolved, permissions).await?;
            }

            // Set times if specified
            // Note: This requires additional platform-specific code
            // For now, we just acknowledge the request

            Ok(Self::status_ok(id))
        }

        async fn fsetstat(
            &mut self,
            id: u32,
            handle: String,
            attrs: FileAttributes,
        ) -> Result<Status, Self::Error> {
            debug!(
                "[{}] SFTP fsetstat: id={}, handle={}, attrs={:?}",
                self.conn_id, id, handle, attrs
            );

            self.check_write()?;

            let handles = self.file_handles.lock().await;
            let fh = handles
                .get(&handle)
                .ok_or_else(|| SftpError::InvalidHandle(handle.clone()))?;

            // Set permissions if specified
            #[cfg(unix)]
            if let Some(perm) = attrs.permissions {
                use std::os::unix::fs::PermissionsExt;
                let permissions = std::fs::Permissions::from_mode(perm);
                fs::set_permissions(&fh.path, permissions).await?;
            }

            Ok(Self::status_ok(id))
        }

        async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
            debug!("[{}] SFTP opendir: id={}, path={}", self.conn_id, id, path);

            self.check_read()?;

            let resolved = self.resolve_path(&path)?;

            if !resolved.is_dir() {
                return Err(SftpError::NotDirectory(path));
            }

            // Read directory entries synchronously (tokio::fs::read_dir is complex to hold)
            let entries: Vec<std::fs::DirEntry> = std::fs::read_dir(&resolved)?
                .filter_map(|e| e.ok())
                .collect();

            let handle_id = self.next_handle_id();

            let mut handles = self.dir_handles.lock().await;
            handles.insert(
                handle_id.clone(),
                DirHandle {
                    path: resolved,
                    entries,
                    position: 0,
                    read_complete: false,
                },
            );

            info!(
                "[{}] Opened directory: {} -> handle {}",
                self.conn_id, path, handle_id
            );

            Ok(Handle {
                id,
                handle: handle_id,
            })
        }

        async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
            debug!(
                "[{}] SFTP readdir: id={}, handle={}",
                self.conn_id, id, handle
            );

            self.check_read()?;

            let mut handles = self.dir_handles.lock().await;
            let dh = handles
                .get_mut(&handle)
                .ok_or_else(|| SftpError::InvalidHandle(handle.clone()))?;

            if dh.read_complete {
                return Err(SftpError::Eof);
            }

            // Return entries in batches
            const BATCH_SIZE: usize = 100;
            let mut files = Vec::new();

            // Add . and .. on first read
            if dh.position == 0 {
                // Add "."
                if let Ok(metadata) = std::fs::metadata(&dh.path) {
                    let attrs = Self::metadata_to_attrs(&metadata);
                    files.push(File::new(".".to_string(), attrs));
                }

                // Add ".."
                if let Some(parent) = dh.path.parent() {
                    if let Ok(metadata) = std::fs::metadata(parent) {
                        let attrs = Self::metadata_to_attrs(&metadata);
                        files.push(File::new("..".to_string(), attrs));
                    }
                }
            }

            // Add regular entries
            while dh.position < dh.entries.len() && files.len() < BATCH_SIZE {
                let entry = &dh.entries[dh.position];
                dh.position += 1;

                if let Ok(metadata) = entry.metadata() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let attrs = Self::metadata_to_attrs(&metadata);
                    files.push(File::new(name, attrs));
                }
            }

            if dh.position >= dh.entries.len() {
                dh.read_complete = true;
            }

            if files.is_empty() {
                return Err(SftpError::Eof);
            }

            Ok(Name { id, files })
        }

        async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
            debug!(
                "[{}] SFTP remove: id={}, filename={}",
                self.conn_id, id, filename
            );

            self.check_write()?;

            let path = self.resolve_path(&filename)?;

            if path.is_dir() {
                return Err(SftpError::IsDirectory(filename));
            }

            fs::remove_file(&path).await?;

            info!("[{}] Removed file: {}", self.conn_id, filename);

            Ok(Self::status_ok(id))
        }

        async fn mkdir(
            &mut self,
            id: u32,
            path: String,
            attrs: FileAttributes,
        ) -> Result<Status, Self::Error> {
            debug!(
                "[{}] SFTP mkdir: id={}, path={}, attrs={:?}",
                self.conn_id, id, path, attrs
            );

            self.check_write()?;

            let resolved = self.resolve_path(&path)?;

            fs::create_dir(&resolved).await?;

            // Set permissions if specified
            #[cfg(unix)]
            if let Some(perm) = attrs.permissions {
                use std::os::unix::fs::PermissionsExt;
                let permissions = std::fs::Permissions::from_mode(perm);
                fs::set_permissions(&resolved, permissions).await?;
            }

            info!("[{}] Created directory: {}", self.conn_id, path);

            Ok(Self::status_ok(id))
        }

        async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
            debug!("[{}] SFTP rmdir: id={}, path={}", self.conn_id, id, path);

            self.check_write()?;

            let resolved = self.resolve_path(&path)?;

            if !resolved.is_dir() {
                return Err(SftpError::NotDirectory(path));
            }

            fs::remove_dir(&resolved).await?;

            info!("[{}] Removed directory: {}", self.conn_id, path);

            Ok(Self::status_ok(id))
        }

        async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
            debug!("[{}] SFTP realpath: id={}, path={}", self.conn_id, id, path);

            let resolved = self.resolve_path(&path)?;
            let canonical = resolved.to_string_lossy().to_string();

            // Return relative to root if root is set
            let display_path = if let Some(ref root) = self.config.root_dir {
                let root_str = root.to_string_lossy();
                if canonical.starts_with(root_str.as_ref()) {
                    let relative = &canonical[root_str.len()..];
                    if relative.is_empty() {
                        "/".to_string()
                    } else if relative.starts_with('/') || relative.starts_with('\\') {
                        relative.to_string()
                    } else {
                        format!("/{}", relative)
                    }
                } else {
                    canonical
                }
            } else {
                canonical
            };

            let attrs = if resolved.exists() {
                let metadata = fs::metadata(&resolved).await?;
                Self::metadata_to_attrs(&metadata)
            } else {
                FileAttributes::default()
            };

            Ok(Name {
                id,
                files: vec![File::new(display_path, attrs)],
            })
        }

        async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
            debug!("[{}] SFTP stat: id={}, path={}", self.conn_id, id, path);

            self.check_read()?;

            let resolved = self.resolve_path(&path)?;
            let metadata = fs::metadata(&resolved).await?;
            let attrs = Self::metadata_to_attrs(&metadata);

            Ok(Attrs { id, attrs })
        }

        async fn rename(
            &mut self,
            id: u32,
            oldpath: String,
            newpath: String,
        ) -> Result<Status, Self::Error> {
            debug!(
                "[{}] SFTP rename: id={}, oldpath={}, newpath={}",
                self.conn_id, id, oldpath, newpath
            );

            self.check_write()?;

            let old_resolved = self.resolve_path(&oldpath)?;
            let new_resolved = self.resolve_path(&newpath)?;

            fs::rename(&old_resolved, &new_resolved).await?;

            info!("[{}] Renamed: {} -> {}", self.conn_id, oldpath, newpath);

            Ok(Self::status_ok(id))
        }

        async fn readlink(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
            debug!("[{}] SFTP readlink: id={}, path={}", self.conn_id, id, path);

            self.check_read()?;

            let resolved = self.resolve_path(&path)?;
            let target = fs::read_link(&resolved).await?;
            let target_str = target.to_string_lossy().to_string();

            Ok(Name {
                id,
                files: vec![File::new(target_str, FileAttributes::default())],
            })
        }

        async fn symlink(
            &mut self,
            id: u32,
            linkpath: String,
            targetpath: String,
        ) -> Result<Status, Self::Error> {
            debug!(
                "[{}] SFTP symlink: id={}, linkpath={}, targetpath={}",
                self.conn_id, id, linkpath, targetpath
            );

            self.check_write()?;

            let link_resolved = self.resolve_path(&linkpath)?;

            // Create symlink
            #[cfg(unix)]
            {
                tokio::fs::symlink(&targetpath, &link_resolved).await?;
            }

            #[cfg(windows)]
            {
                // On Windows, we need to know if target is a file or directory
                let target_path = PathBuf::from(&targetpath);
                if target_path.is_dir() {
                    tokio::fs::symlink_dir(&targetpath, &link_resolved).await?;
                } else {
                    tokio::fs::symlink_file(&targetpath, &link_resolved).await?;
                }
            }

            info!(
                "[{}] Created symlink: {} -> {}",
                self.conn_id, linkpath, targetpath
            );

            Ok(Self::status_ok(id))
        }

        async fn extended(
            &mut self,
            id: u32,
            request: String,
            _data: Vec<u8>,
        ) -> Result<Packet, Self::Error> {
            debug!(
                "[{}] SFTP extended: id={}, request={}",
                self.conn_id, id, request
            );

            // We don't support any extensions yet
            Err(SftpError::NotSupported)
        }
    }
}

// Re-export server types when sftp feature is enabled
#[cfg(feature = "sftp")]
pub use server_impl::{SftpError, SftpHandler};

// SFTP Client implementation (feature-gated)
#[cfg(feature = "sftp")]
mod client_impl {
    use super::*;
    use russh_sftp::client::SftpSession;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// SFTP client for file transfers
    pub struct SftpClient {
        session: SftpSession,
    }

    impl SftpClient {
        /// Create a new SFTP client from a russh channel
        pub async fn new(channel: russh::Channel<russh::client::Msg>) -> anyhow::Result<Self> {
            let session = SftpSession::new(channel.into_stream()).await?;
            Ok(Self { session })
        }

        /// Download a file from remote to local
        pub async fn get(&self, remote: &str, local: &str) -> anyhow::Result<u64> {
            info!("SFTP get: {} -> {}", remote, local);

            let mut remote_file = self.session.open(remote).await?;
            let mut local_file = tokio::fs::File::create(local).await?;

            let mut buffer = vec![0u8; 32768];
            let mut total_bytes = 0u64;

            loop {
                let n = remote_file.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }
                local_file.write_all(&buffer[..n]).await?;
                total_bytes += n as u64;
            }

            info!("SFTP get complete: {} bytes", total_bytes);
            Ok(total_bytes)
        }

        /// Upload a file from local to remote
        pub async fn put(&self, local: &str, remote: &str) -> anyhow::Result<u64> {
            info!("SFTP put: {} -> {}", local, remote);

            let mut local_file = tokio::fs::File::open(local).await?;
            let mut remote_file = self.session.create(remote).await?;

            let mut buffer = vec![0u8; 32768];
            let mut total_bytes = 0u64;

            loop {
                let n = local_file.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }
                remote_file.write_all(&buffer[..n]).await?;
                total_bytes += n as u64;
            }

            info!("SFTP put complete: {} bytes", total_bytes);
            Ok(total_bytes)
        }

        /// List directory contents
        pub async fn ls(&self, path: &str) -> anyhow::Result<Vec<FileInfo>> {
            debug!("SFTP ls: {}", path);

            let entries = self.session.read_dir(path).await?;
            let mut result = Vec::new();

            for entry in entries {
                let name = entry.file_name();
                let attrs = entry.metadata();

                result.push(FileInfo {
                    name,
                    size: attrs.size.unwrap_or(0),
                    is_dir: attrs.is_dir(),
                    modified: attrs.mtime.unwrap_or(0) as u64,
                    permissions: attrs.permissions,
                });
            }

            Ok(result)
        }

        /// Create a directory
        pub async fn mkdir(&self, path: &str) -> anyhow::Result<()> {
            debug!("SFTP mkdir: {}", path);
            self.session.create_dir(path).await?;
            Ok(())
        }

        /// Remove a file
        pub async fn rm(&self, path: &str) -> anyhow::Result<()> {
            debug!("SFTP rm: {}", path);
            self.session.remove_file(path).await?;
            Ok(())
        }

        /// Remove a directory
        pub async fn rmdir(&self, path: &str) -> anyhow::Result<()> {
            debug!("SFTP rmdir: {}", path);
            self.session.remove_dir(path).await?;
            Ok(())
        }

        /// Get file/directory attributes
        pub async fn stat(&self, path: &str) -> anyhow::Result<FileInfo> {
            debug!("SFTP stat: {}", path);

            let attrs = self.session.metadata(path).await?;

            Ok(FileInfo {
                name: Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                size: attrs.size.unwrap_or(0),
                is_dir: attrs.is_dir(),
                modified: attrs.mtime.unwrap_or(0) as u64,
                permissions: attrs.permissions,
            })
        }

        /// Rename/move a file or directory
        pub async fn rename(&self, oldpath: &str, newpath: &str) -> anyhow::Result<()> {
            debug!("SFTP rename: {} -> {}", oldpath, newpath);
            self.session.rename(oldpath, newpath).await?;
            Ok(())
        }

        /// Get the canonicalized absolute path
        pub async fn realpath(&self, path: &str) -> anyhow::Result<String> {
            debug!("SFTP realpath: {}", path);
            let canonical = self.session.canonicalize(path).await?;
            Ok(canonical)
        }

        /// Close the SFTP session
        pub async fn close(self) -> anyhow::Result<()> {
            self.session.close().await?;
            Ok(())
        }
    }
}

#[cfg(feature = "sftp")]
pub use client_impl::SftpClient;

// Stub implementations when sftp feature is disabled
#[cfg(not(feature = "sftp"))]
pub struct SftpClient;

#[cfg(not(feature = "sftp"))]
impl SftpClient {
    pub async fn get(&self, _remote: &str, _local: &str) -> anyhow::Result<u64> {
        Err(anyhow::anyhow!("SFTP feature not enabled"))
    }

    pub async fn put(&self, _local: &str, _remote: &str) -> anyhow::Result<u64> {
        Err(anyhow::anyhow!("SFTP feature not enabled"))
    }

    pub async fn ls(&self, _path: &str) -> anyhow::Result<Vec<FileInfo>> {
        Err(anyhow::anyhow!("SFTP feature not enabled"))
    }

    pub async fn mkdir(&self, _path: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("SFTP feature not enabled"))
    }

    pub async fn rm(&self, _path: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("SFTP feature not enabled"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sftp_config_default() {
        let config = SftpServerConfig::default();
        assert!(config.allow_read);
        assert!(config.allow_write);
        assert!(config.root_dir.is_none());
    }

    #[test]
    fn test_sftp_config_read_only() {
        let config = SftpServerConfig::read_only(Some(PathBuf::from("/tmp")));
        assert!(config.allow_read);
        assert!(!config.allow_write);
        assert_eq!(config.root_dir, Some(PathBuf::from("/tmp")));
    }
}
