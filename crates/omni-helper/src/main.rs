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
    use tokio::sync::broadcast;
    let (tx, _rx) = broadcast::channel(1);
    let tx_stop = tx.clone();

    let service_handler = move |event| match event {
        ServiceControl::Stop => {
            info!("Received STOP signal from Windows Service Control");
            let _ = tx_stop.send(());
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
        if let Err(e) = run_helper_server(tx.subscribe()).await {
            error!("Helper server error: {}", e);
        }
    });

    info!("Service loop finished, stopping service...");

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

async fn run_helper_server(
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    info!("OmniEdge Helper server starting...");
    let base_url = omni_core::config::get_api_base_url();
    info!("Using API base URL: {}", base_url);
    let server = Arc::new(HelperServer::new(base_url));

    #[cfg(unix)]
    {
        let socket_path = "/var/run/omniedge-helper.sock";
        let path = std::path::Path::new(socket_path);

        if path.exists() {
            let _ = std::fs::remove_file(path);
        }

        let listener = tokio::net::UnixListener::bind(socket_path)?;

        // Set socket permissions to allow access by all local users
        // Security note: The helper validates all commands and only accepts
        // specific operations (connect, disconnect, status). The socket being
        // world-accessible is intentional for a VPN service that unprivileged
        // users need to control. Commands are validated in handle_request().
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(socket_path) {
            let mut perms = metadata.permissions();
            // 0o666 allows any local user to connect - this is required for
            // non-root users to control the VPN. The helper only accepts
            // authenticated requests with valid tokens.
            perms.set_mode(0o666);
            let _ = std::fs::set_permissions(socket_path, perms);
        }

        info!("Listening on Unix Socket: {}", socket_path);

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((socket, _)) => {
                            let server_ref = Arc::clone(&server);
                            tokio::spawn(async move {
                                handle_connection(socket, server_ref).await;
                            });
                        }
                        Err(e) => error!("Accept error: {}", e),
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, closing Unix socket listener...");
                    break;
                }
            }
        }
    }

    #[cfg(windows)]
    {
        let pipe_name = r"\\.\pipe\omniedge-helper";
        info!("Listening on Named Pipe: {}", pipe_name);

        let mut first = true;
        loop {
            // Create a pipe instance for this iteration
            let server_instance = match create_permissive_pipe(pipe_name, first) {
                Ok(s) => s,
                Err(e) => {
                    error!("CreateNamedPipe error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }
            };
            first = false;

            tokio::select! {
                connect_res = server_instance.connect() => {
                    if connect_res.is_ok() {
                        let server_ref = Arc::clone(&server);
                        // IMPORTANT: Spawn the connection handler so we can immediately
                        // create a new pipe instance for the next client. This prevents
                        // "All pipe instances are busy" errors when multiple requests
                        // come in rapid succession.
                        tokio::spawn(async move {
                            handle_connection(server_instance, server_ref).await;
                        });
                    }
                    // Loop immediately continues to create new pipe instance
                }
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, closing Named Pipe listener...");
                    break;
                }
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn create_permissive_pipe(
    name: &str,
    first: bool,
) -> anyhow::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
    use winapi::um::namedpipeapi::CreateNamedPipeW;
    use winapi::um::securitybaseapi::{InitializeSecurityDescriptor, SetSecurityDescriptorDacl};
    use winapi::um::winbase::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
        PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };
    use winapi::um::winnt::SECURITY_DESCRIPTOR;

    let name_wide: Vec<u16> = std::ffi::OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Security: Create a NULL DACL to allow any local user to connect.
    // This is intentional for a VPN helper service that unprivileged users
    // need to control. Security is enforced at the command level:
    // - Only specific commands are accepted (connect, disconnect, status)
    // - Connect requires valid authentication tokens
    // - All requests are validated in HelperServer::handle_request()
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

/// Maximum buffer size for request/response messages (16KB).
/// This should be sufficient for all helper commands.
const MAX_MESSAGE_SIZE: usize = 16 * 1024;

/// Read timeout for client connections (30 seconds).
/// Prevents blocking if a client connects but never sends data.
const READ_TIMEOUT_SECS: u64 = 30;

/// Handle a single request on a connection and return.
///
/// On Windows, this design allows the pipe server to quickly create new pipe
/// instances for other clients. The desktop app creates a new connection for
/// each request, so single-request handling is sufficient and avoids the
/// "All pipe instances are busy" error that occurs when handle_connection
/// blocks in a loop.
///
/// Security note: This function only handles message parsing and response.
/// Command authorization is enforced in `HelperServer::handle_request()`.
async fn handle_connection<S>(mut socket: S, server: Arc<HelperServer>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::time::{timeout, Duration};

    let mut buf = [0u8; MAX_MESSAGE_SIZE];

    // Read a single request with timeout to prevent indefinite blocking
    let read_result = timeout(
        Duration::from_secs(READ_TIMEOUT_SECS),
        socket.read(&mut buf),
    )
    .await;

    let n = match read_result {
        Ok(Ok(0)) => return, // Client closed connection
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            error!("Read error: {}", e);
            return;
        }
        Err(_) => {
            error!(
                "Read timeout after {}s - client did not send data",
                READ_TIMEOUT_SECS
            );
            return;
        }
    };

    // Check if buffer might have been too small (message truncated)
    if n == MAX_MESSAGE_SIZE {
        error!(
            "Request may be truncated (received max buffer size {})",
            MAX_MESSAGE_SIZE
        );
        let _ = send_error_response(&mut socket, "Request too large").await;
        return;
    }

    let req: HelperRequest = match serde_json::from_slice(&buf[..n]) {
        Ok(r) => r,
        Err(e) => {
            // Log detailed error but send generic message to client
            error!("JSON parse error: {} (received {} bytes)", e, n);
            let _ = send_error_response(&mut socket, "Invalid request format").await;
            return;
        }
    };

    let resp = server.handle_request(req).await;

    // Serialize response with fallback for unlikely serialization errors
    let resp_bytes = match serde_json::to_vec(&resp) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Response serialization error: {}", e);
            b"{\"success\":false,\"message\":\"Internal error\",\"data\":null}".to_vec()
        }
    };

    if let Err(e) = socket.write_all(&resp_bytes).await {
        error!("Write error: {}", e);
    }
    // Connection closes after response - client must reconnect for next request
}

/// Send an error response to the client.
/// Uses a pre-formatted JSON string to avoid serialization in error paths.
async fn send_error_response<S>(socket: &mut S, message: &str) -> Result<(), std::io::Error>
where
    S: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;

    // Escape message for JSON (basic escaping for quotes and backslashes)
    let escaped_message: String = message
        .chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            _ => vec![c],
        })
        .collect();

    let error_json = format!(
        r#"{{"success":false,"message":"{}","data":null}}"#,
        escaped_message
    );

    socket.write_all(error_json.as_bytes()).await
}

fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

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
            info!("Failed to start as service (expected if running from terminal), falling back to console mode");
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let (tx, _rx) = tokio::sync::broadcast::channel(1);
                let tx_stop = tx.clone();

                tokio::spawn(async move {
                    if tokio::signal::ctrl_c().await.is_ok() {
                        info!("Received Ctrl+C, shutting down helper...");
                        let _ = tx_stop.send(());
                    }
                });

                run_helper_server(tx.subscribe()).await
            })?;
        }
    }

    #[cfg(not(windows))]
    {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let (tx, _rx) = tokio::sync::broadcast::channel(1);
            let tx_stop = tx.clone();

            use tokio::signal::unix::{signal, SignalKind};
            let mut sigint = signal(SignalKind::interrupt())?;
            let mut sigterm = signal(SignalKind::terminate())?;

            tokio::spawn(async move {
                tokio::select! {
                    _ = sigint.recv() => info!("Received SIGINT, shutting down..."),
                    _ = sigterm.recv() => info!("Received SIGTERM, shutting down..."),
                }
                let _ = tx_stop.send(());
            });

            run_helper_server(tx.subscribe()).await
        })?;
    }

    info!("OmniEdge Helper stopped clean.");
    Ok(())
}
