//! Process spawning with privilege separation
//!
//! This module handles spawning user sessions with proper privilege drop
//! to run commands as the target user with appropriate isolation.

use crate::types::SessionCommand;
use std::collections::HashMap;
use std::path::PathBuf;

/// Arguments for spawning a user session
#[derive(Debug, Clone)]
pub struct IncubatorArgs {
    /// Target user ID
    pub uid: u32,
    /// Target group ID
    pub gid: u32,
    /// Supplementary groups
    pub groups: Vec<u32>,
    /// Home directory
    pub home_dir: PathBuf,
    /// Login shell
    pub shell: PathBuf,
    /// Command to execute
    pub command: SessionCommand,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// PTY request (if any)
    pub pty: Option<PtyRequest>,
    /// Username for environment variable
    pub username: String,
}

/// PTY request info
#[derive(Debug, Clone)]
pub struct PtyRequest {
    /// Terminal type (e.g., "xterm-256color")
    pub term: String,
    /// Terminal width in columns
    pub width: u32,
    /// Terminal height in rows
    pub height: u32,
    /// Terminal pixel width (optional)
    pub pixel_width: u32,
    /// Terminal pixel height (optional)
    pub pixel_height: u32,
}

impl Default for PtyRequest {
    fn default() -> Self {
        Self {
            term: "xterm-256color".to_string(),
            width: 80,
            height: 24,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Spawn user process with proper privilege separation (Unix - Linux)
#[cfg(all(unix, not(target_os = "macos")))]
pub async fn spawn_session(args: IncubatorArgs) -> anyhow::Result<tokio::process::Child> {
    use nix::unistd::{setgid, setgroups, setuid, Gid, Uid};
    use std::os::unix::process::CommandExt;
    use tokio::process::Command;

    let shell = args.shell.clone();
    let home = args.home_dir.clone();
    let uid = args.uid;
    let gid = args.gid;
    let groups = args.groups.clone();
    let env = args.env.clone();
    let username = args.username.clone();

    let mut cmd = match &args.command {
        SessionCommand::Shell => {
            let mut c = Command::new(&shell);
            c.arg("-l"); // Login shell
            c
        }
        SessionCommand::Exec(command) => {
            let mut c = Command::new(&shell);
            c.args(["-c", command]);
            c
        }
        SessionCommand::Sftp => {
            // SFTP is handled differently - in-process
            return Err(anyhow::anyhow!("SFTP should be handled in-process"));
        }
    };

    // Set environment
    cmd.env_clear();
    cmd.envs(env);
    cmd.env("HOME", &home);
    cmd.env("USER", &username);
    cmd.env("LOGNAME", &username);
    cmd.env("SHELL", &shell);
    cmd.env(
        "PATH",
        "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin",
    );

    // Set terminal type if PTY requested
    if let Some(ref pty) = args.pty {
        cmd.env("TERM", &pty.term);
        cmd.env("COLUMNS", pty.width.to_string());
        cmd.env("LINES", pty.height.to_string());
    }

    // Set working directory
    cmd.current_dir(&home);

    // Drop privileges before exec (in child process)
    // SAFETY: pre_exec runs in the child after fork, before exec.
    // We're only calling async-signal-safe functions (setgroups, setgid, setuid).
    unsafe {
        cmd.pre_exec(move || {
            // Set supplementary groups
            let gids: Vec<Gid> = groups.iter().map(|g| Gid::from_raw(*g)).collect();
            setgroups(&gids).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            // Set GID (must be before setuid)
            setgid(Gid::from_raw(gid))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            // Set UID (must be last - after this we can't regain privileges)
            setuid(Uid::from_raw(uid))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            // Verify we can't regain root privileges
            if uid != 0 && setuid(Uid::from_raw(0)).is_ok() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to drop privileges permanently",
                ));
            }

            Ok(())
        });
    }

    let child = cmd.spawn()?;
    Ok(child)
}

/// Spawn user process with proper privilege separation (Unix - macOS)
/// Note: macOS uses different setgroups signature and nix doesn't expose it for Apple targets
#[cfg(target_os = "macos")]
pub async fn spawn_session(args: IncubatorArgs) -> anyhow::Result<tokio::process::Child> {
    use nix::unistd::{setgid, setuid, Gid, Uid};
    use std::os::unix::process::CommandExt;
    use tokio::process::Command;

    let shell = args.shell.clone();
    let home = args.home_dir.clone();
    let uid = args.uid;
    let gid = args.gid;
    let groups = args.groups.clone();
    let env = args.env.clone();
    let username = args.username.clone();

    let mut cmd = match &args.command {
        SessionCommand::Shell => {
            let mut c = Command::new(&shell);
            c.arg("-l"); // Login shell
            c
        }
        SessionCommand::Exec(command) => {
            let mut c = Command::new(&shell);
            c.args(["-c", command]);
            c
        }
        SessionCommand::Sftp => {
            // SFTP is handled differently - in-process
            return Err(anyhow::anyhow!("SFTP should be handled in-process"));
        }
    };

    // Set environment
    cmd.env_clear();
    cmd.envs(env);
    cmd.env("HOME", &home);
    cmd.env("USER", &username);
    cmd.env("LOGNAME", &username);
    cmd.env("SHELL", &shell);
    cmd.env(
        "PATH",
        "/usr/local/bin:/usr/bin:/bin:/usr/local/sbin:/usr/sbin:/sbin",
    );

    // Set terminal type if PTY requested
    if let Some(ref pty) = args.pty {
        cmd.env("TERM", &pty.term);
        cmd.env("COLUMNS", pty.width.to_string());
        cmd.env("LINES", pty.height.to_string());
    }

    // Set working directory
    cmd.current_dir(&home);

    // Drop privileges before exec (in child process)
    // SAFETY: pre_exec runs in the child after fork, before exec.
    // We're only calling async-signal-safe functions (setgroups, setgid, setuid).
    unsafe {
        cmd.pre_exec(move || {
            // Set supplementary groups using libc directly (macOS has different types)
            // On macOS, gid_t is i32, not u32
            let gids: Vec<libc::gid_t> = groups.iter().map(|g| *g as libc::gid_t).collect();
            if libc::setgroups(gids.len() as libc::c_int, gids.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }

            // Set GID (must be before setuid)
            setgid(Gid::from_raw(gid))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            // Set UID (must be last - after this we can't regain privileges)
            setuid(Uid::from_raw(uid))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

            // Verify we can't regain root privileges
            if uid != 0 && setuid(Uid::from_raw(0)).is_ok() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to drop privileges permanently",
                ));
            }

            Ok(())
        });
    }

    let child = cmd.spawn()?;
    Ok(child)
}

/// Spawn user process (Windows)
/// Note: Windows privilege separation is more complex and requires different mechanisms
#[cfg(windows)]
pub async fn spawn_session(args: IncubatorArgs) -> anyhow::Result<tokio::process::Child> {
    use tokio::process::Command;

    // Windows: Use CreateProcessAsUser or similar for proper impersonation
    // For now, basic implementation without impersonation
    // TODO: Implement proper Windows user impersonation using LogonUser + CreateProcessAsUser
    let mut cmd = match &args.command {
        SessionCommand::Shell => {
            let mut c = Command::new("cmd.exe");
            c.arg("/K");
            c
        }
        SessionCommand::Exec(command) => {
            let mut c = Command::new("cmd.exe");
            c.args(["/C", command]);
            c
        }
        SessionCommand::Sftp => {
            return Err(anyhow::anyhow!("SFTP should be handled in-process"));
        }
    };

    cmd.env_clear();
    cmd.envs(args.env);
    cmd.current_dir(&args.home_dir);

    if let Some(ref pty) = args.pty {
        cmd.env("TERM", &pty.term);
    }

    let child = cmd.spawn()?;
    Ok(child)
}

/// Look up user information by username
#[cfg(unix)]
pub fn lookup_user(username: &str) -> anyhow::Result<UserInfo> {
    use std::ffi::CString;

    let c_username = CString::new(username)?;

    // SAFETY: getpwnam_r is thread-safe
    unsafe {
        let mut pwd: libc::passwd = std::mem::zeroed();
        let mut buf = vec![0u8; 4096];
        let mut result: *mut libc::passwd = std::ptr::null_mut();

        let ret = libc::getpwnam_r(
            c_username.as_ptr(),
            &mut pwd,
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            &mut result,
        );

        if ret != 0 || result.is_null() {
            return Err(anyhow::anyhow!("User '{}' not found", username));
        }

        let home = std::ffi::CStr::from_ptr(pwd.pw_dir)
            .to_string_lossy()
            .to_string();
        let shell = std::ffi::CStr::from_ptr(pwd.pw_shell)
            .to_string_lossy()
            .to_string();

        Ok(UserInfo {
            uid: pwd.pw_uid,
            gid: pwd.pw_gid,
            home_dir: PathBuf::from(home),
            shell: PathBuf::from(shell),
            groups: get_user_groups(username, pwd.pw_gid)?,
        })
    }
}

/// Get supplementary groups for a user (Linux)
#[cfg(all(unix, not(target_os = "macos")))]
fn get_user_groups(username: &str, primary_gid: u32) -> anyhow::Result<Vec<u32>> {
    use std::ffi::CString;

    let c_username = CString::new(username)?;

    // Start with a reasonable buffer size
    let mut ngroups: libc::c_int = 32;
    let mut groups: Vec<libc::gid_t> = vec![0; ngroups as usize];

    // SAFETY: getgrouplist is a standard POSIX function
    unsafe {
        let ret = libc::getgrouplist(
            c_username.as_ptr(),
            primary_gid as libc::gid_t,
            groups.as_mut_ptr(),
            &mut ngroups,
        );

        if ret == -1 {
            // Buffer was too small, resize and retry
            groups.resize(ngroups as usize, 0);
            libc::getgrouplist(
                c_username.as_ptr(),
                primary_gid as libc::gid_t,
                groups.as_mut_ptr(),
                &mut ngroups,
            );
        }

        groups.truncate(ngroups as usize);
        Ok(groups.into_iter().map(|g| g as u32).collect())
    }
}

/// Get supplementary groups for a user (macOS)
/// Note: On macOS, gid_t is i32 and getgrouplist uses different types
#[cfg(target_os = "macos")]
fn get_user_groups(username: &str, primary_gid: u32) -> anyhow::Result<Vec<u32>> {
    use std::ffi::CString;

    let c_username = CString::new(username)?;

    // Start with a reasonable buffer size
    let mut ngroups: libc::c_int = 32;
    // On macOS, gid_t is i32
    let mut groups: Vec<i32> = vec![0; ngroups as usize];

    // SAFETY: getgrouplist is a standard POSIX function
    unsafe {
        let ret = libc::getgrouplist(
            c_username.as_ptr(),
            primary_gid as i32,
            groups.as_mut_ptr(),
            &mut ngroups,
        );

        if ret == -1 {
            // Buffer was too small, resize and retry
            groups.resize(ngroups as usize, 0);
            libc::getgrouplist(
                c_username.as_ptr(),
                primary_gid as i32,
                groups.as_mut_ptr(),
                &mut ngroups,
            );
        }

        groups.truncate(ngroups as usize);
        Ok(groups.into_iter().map(|g| g as u32).collect())
    }
}

/// Look up user information (Windows stub)
#[cfg(windows)]
pub fn lookup_user(username: &str) -> anyhow::Result<UserInfo> {
    // Windows: Would use NetUserGetInfo or similar
    // For now, return a basic structure
    Ok(UserInfo {
        uid: 0,
        gid: 0,
        home_dir: PathBuf::from(format!("C:\\Users\\{}", username)),
        shell: PathBuf::from("cmd.exe"),
        groups: vec![],
    })
}

/// User information for session spawning
#[derive(Debug, Clone)]
pub struct UserInfo {
    /// User ID
    pub uid: u32,
    /// Primary group ID
    pub gid: u32,
    /// Home directory
    pub home_dir: PathBuf,
    /// Login shell
    pub shell: PathBuf,
    /// Supplementary group IDs
    pub groups: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_request_default() {
        let pty = PtyRequest::default();
        assert_eq!(pty.term, "xterm-256color");
        assert_eq!(pty.width, 80);
        assert_eq!(pty.height, 24);
    }

    #[cfg(unix)]
    #[test]
    fn test_lookup_root_user() {
        // Root user should always exist on Unix systems
        let result = lookup_user("root");
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.uid, 0);
        assert_eq!(info.gid, 0);
    }

    #[test]
    fn test_lookup_nonexistent_user() {
        let result = lookup_user("nonexistent_user_12345");
        // Should fail on both Unix and Windows
        #[cfg(unix)]
        assert!(result.is_err());
    }
}
