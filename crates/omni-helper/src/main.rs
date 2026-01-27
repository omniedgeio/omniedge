use log::{error, info};
use omni_helper::{HelperRequest, HelperServer};
use std::sync::Arc;

#[cfg(windows)]
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self},
    service_dispatcher,
};

#[cfg(windows)]
define_windows_service!(ffi_service_main, service_main);

#[cfg(windows)]
fn service_main(_arguments: Vec<std::ffi::OsString>) {
    if let Err(_e) = run_service() {
        // Handle error
    }
}

#[cfg(windows)]
fn run_service() -> anyhow::Result<()> {
    let service_handler = move |event| match event {
        ServiceControl::Stop => {
            windows_service::service_control_handler::ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => {
            windows_service::service_control_handler::ServiceControlHandlerResult::NoError
        }
        _ => windows_service::service_control_handler::ServiceControlHandlerResult::NotImplemented,
    };

    let status_handle = service_control_handler::register("OmniEdgeHelper", service_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(5),
        process_id: None,
    })?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        if let Err(e) = run_helper_server().await {
            error!("Helper server error: {}", e);
        }
    });

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

async fn run_helper_server() -> anyhow::Result<()> {
    info!("OmniEdge Helper server starting...");
    let server = Arc::new(HelperServer::new(
        "https://api.omniedge.io/api/v2".to_string(),
    ));

    #[cfg(unix)]
    {
        let socket_path = "/var/run/omniedge-helper.sock";
        let path = std::path::Path::new(socket_path);

        if path.exists() {
            let _ = std::fs::remove_file(path);
        }

        let listener = tokio::net::UnixListener::bind(socket_path)?;

        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(socket_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o666);
            let _ = std::fs::set_permissions(socket_path, perms);
        }

        info!("Listening on Unix Socket: {}", socket_path);

        loop {
            match listener.accept().await {
                Ok((socket, _)) => {
                    let server_ref = Arc::clone(&server);
                    tokio::spawn(async move {
                        handle_connection(socket, server_ref).await;
                    });
                }
                Err(e) => error!("Accept error: {}", e),
            }
        }
    }

    #[cfg(windows)]
    {
        let pipe_name = r"\\.\pipe\omniedge-helper";
        info!("Listening on Named Pipe: {}", pipe_name);

        let mut first = true;
        loop {
            let server_instance = create_permissive_pipe(pipe_name, first);

            let server_instance = match server_instance {
                Ok(s) => s,
                Err(e) => {
                    error!("CreateNamedPipe error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            if server_instance.connect().await.is_ok() {
                let server_ref = Arc::clone(&server);
                tokio::spawn(async move {
                    handle_connection(server_instance, server_ref).await;
                });
            }
            first = false;
        }
    }
}

#[cfg(windows)]
fn create_permissive_pipe(
    name: &str,
    first: bool,
) -> anyhow::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use std::ptr;
    use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
    use winapi::um::namedpipeapi::CreateNamedPipeW;
    use winapi::um::securitybaseapi::{InitializeSecurityDescriptor, SetSecurityDescriptorDacl};
    use winapi::um::winbase::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
        PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use winapi::um::winnt::{PSECURITY_DESCRIPTOR, SECURITY_DESCRIPTOR};

    let name_wide: Vec<u16> = std::ffi::OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut sd = unsafe { std::mem::zeroed::<SECURITY_DESCRIPTOR>() };
    unsafe {
        InitializeSecurityDescriptor(
            &mut sd as *mut _ as *mut _,
            winapi::um::winnt::SECURITY_DESCRIPTOR_REVISION,
        );
        SetSecurityDescriptorDacl(&mut sd as *mut _ as *mut _, 1, ptr::null_mut(), 0);
    }

    let mut sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: &mut sd as *mut _ as *mut _,
        bInheritHandle: 0,
    };

    let mut open_mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
    if first {
        open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }

    let handle = unsafe {
        CreateNamedPipeW(
            name_wide.as_ptr(),
            open_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            4096,
            4096,
            0,
            &mut sa,
        )
    };

    if handle == winapi::um::handleapi::INVALID_HANDLE_VALUE {
        return Err(anyhow::anyhow!(
            "CreateNamedPipeW failed for {}: {}",
            name,
            std::io::Error::last_os_error()
        ));
    }

    unsafe {
        tokio::net::windows::named_pipe::NamedPipeServer::from_raw_handle(handle as *mut _)
            .map_err(|e| anyhow::anyhow!("from_raw_handle failed: {}", e))
    }
}

async fn handle_connection<S>(mut socket: S, server: Arc<HelperServer>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = [0; 4096];
    loop {
        let n = match socket.read(&mut buf).await {
            Ok(0) => return,
            Ok(n) => n,
            Err(e) => {
                error!("Read error: {}", e);
                return;
            }
        };

        let req: HelperRequest = match serde_json::from_slice(&buf[..n]) {
            Ok(r) => r,
            Err(e) => {
                error!("Unmarshal error: {}", e);
                continue;
            }
        };

        let resp = server.handle_request(req).await;
        let resp_bytes = serde_json::to_vec(&resp).unwrap();
        if let Err(e) = socket.write_all(&resp_bytes).await {
            error!("Write error: {}", e);
            return;
        }
    }
}

fn main() -> anyhow::Result<()> {
    // Determine log directory. For service, we might want a system-wide path.
    #[cfg(windows)]
    let log_dir = std::env::var("PROGRAMDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("OmniEdge")
        .join("logs");
    #[cfg(not(windows))]
    let log_dir = std::path::PathBuf::from("/var/log/omniedge");

    let _ = std::fs::create_dir_all(&log_dir);

    let _logger = flexi_logger::Logger::try_with_str(
        "info, omni_core=debug, omni_api=debug, omni_helper=debug",
    )?
    .log_to_file(
        flexi_logger::FileSpec::default()
            .directory(&log_dir)
            .basename("helper"),
    )
    .duplicate_to_stderr(flexi_logger::Duplicate::All)
    .start()?;

    info!("OmniEdge Helper starting (Log dir: {})", log_dir.display());

    #[cfg(windows)]
    {
        // Try to start as a service. If it fails (e.g. running from console), run normally.
        if let Err(_e) = service_dispatcher::start("OmniEdgeHelper", ffi_service_main) {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async { run_helper_server().await })?;
        }
    }

    #[cfg(not(windows))]
    {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { run_helper_server().await })?;
    }

    Ok(())
}
