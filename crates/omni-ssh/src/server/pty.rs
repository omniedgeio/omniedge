//! PTY (Pseudo-Terminal) handling for SSH sessions
//!
//! This module provides cross-platform PTY support for SSH sessions,
//! allowing interactive shell sessions with proper terminal emulation.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use tokio::sync::mpsc;

/// PTY configuration for a session
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Terminal type (e.g., "xterm-256color")
    pub term: String,
    /// Terminal width in columns
    pub cols: u16,
    /// Terminal height in rows
    pub rows: u16,
    /// Terminal pixel width (optional)
    pub pixel_width: u16,
    /// Terminal pixel height (optional)
    pub pixel_height: u16,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            term: "xterm-256color".to_string(),
            cols: 80,
            rows: 24,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

impl From<PtyConfig> for PtySize {
    fn from(config: PtyConfig) -> Self {
        PtySize {
            rows: config.rows,
            cols: config.cols,
            pixel_width: config.pixel_width,
            pixel_height: config.pixel_height,
        }
    }
}

/// A PTY session for an SSH connection
pub struct PtySession {
    /// Master side of the PTY
    master: Box<dyn portable_pty::MasterPty + Send>,
    /// Child process
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Reader for PTY output
    reader: Box<dyn Read + Send>,
    /// Writer for PTY input
    writer: Box<dyn Write + Send>,
}

impl PtySession {
    /// Create a new PTY session with the given shell
    pub fn new(
        shell: &str,
        config: PtyConfig,
        env: Vec<(String, String)>,
        working_dir: Option<&str>,
    ) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let term = config.term.clone();
        let pair = pty_system.openpty(config.into())?;

        let mut cmd = CommandBuilder::new(shell);
        cmd.arg("-l"); // Login shell

        // Set environment
        cmd.env("TERM", &term);
        for (key, value) in env {
            cmd.env(&key, &value);
        }

        // Set working directory
        if let Some(dir) = working_dir {
            cmd.cwd(dir);
        }

        let child = pair.slave.spawn_command(cmd)?;

        // Get reader and writer from master
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            master: pair.master,
            child,
            reader,
            writer,
        })
    }

    /// Create a new PTY session for executing a specific command
    pub fn exec(
        shell: &str,
        command: &str,
        config: PtyConfig,
        env: Vec<(String, String)>,
        working_dir: Option<&str>,
    ) -> anyhow::Result<Self> {
        let pty_system = native_pty_system();
        let term = config.term.clone();
        let pair = pty_system.openpty(config.into())?;

        let mut cmd = CommandBuilder::new(shell);
        cmd.args(["-c", command]);

        // Set environment
        cmd.env("TERM", &term);
        for (key, value) in env {
            cmd.env(&key, &value);
        }

        // Set working directory
        if let Some(dir) = working_dir {
            cmd.cwd(dir);
        }

        let child = pair.slave.spawn_command(cmd)?;

        // Get reader and writer from master
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        Ok(Self {
            master: pair.master,
            child,
            reader,
            writer,
        })
    }

    /// Resize the PTY
    pub fn resize(&self, cols: u16, rows: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Write data to the PTY (input from SSH client)
    pub fn write(&mut self, data: &[u8]) -> anyhow::Result<usize> {
        Ok(self.writer.write(data)?)
    }

    /// Read data from the PTY (output to SSH client)
    pub fn read(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        Ok(self.reader.read(buf)?)
    }

    /// Check if the child process has exited
    pub fn try_wait(&mut self) -> anyhow::Result<Option<portable_pty::ExitStatus>> {
        Ok(self.child.try_wait()?)
    }

    /// Wait for the child process to exit
    pub fn wait(&mut self) -> anyhow::Result<portable_pty::ExitStatus> {
        Ok(self.child.wait()?)
    }

    /// Kill the child process
    pub fn kill(&mut self) -> anyhow::Result<()> {
        self.child.kill()?;
        Ok(())
    }
}

/// Async PTY session wrapper
pub struct AsyncPtySession {
    /// Channel to send input to the PTY
    input_tx: mpsc::Sender<Vec<u8>>,
    /// Channel to receive output from the PTY
    output_rx: mpsc::Receiver<Vec<u8>>,
    /// Channel to receive exit status
    exit_rx: mpsc::Receiver<i32>,
    /// Handle to the PTY task
    _task_handle: tokio::task::JoinHandle<()>,
}

impl AsyncPtySession {
    /// Create a new async PTY session
    pub fn new(
        shell: &str,
        config: PtyConfig,
        env: Vec<(String, String)>,
        working_dir: Option<String>,
    ) -> anyhow::Result<Self> {
        let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(256);
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>(256);
        let (exit_tx, exit_rx) = mpsc::channel::<i32>(1);

        let mut pty = PtySession::new(shell, config, env, working_dir.as_deref())?;

        // Spawn task to handle PTY I/O
        let task_handle = tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 4096];

            loop {
                // Check for input
                if let Ok(data) = input_rx.try_recv() {
                    if let Err(e) = pty.write(&data) {
                        tracing::error!("PTY write error: {}", e);
                        break;
                    }
                }

                // Check for output
                match pty.read(&mut buf) {
                    Ok(0) => {
                        // EOF
                        break;
                    }
                    Ok(n) => {
                        if output_tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if e.to_string().contains("would block") {
                            // Non-blocking read returned no data
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        } else {
                            tracing::error!("PTY read error: {}", e);
                            break;
                        }
                    }
                }

                // Check if child has exited
                match pty.try_wait() {
                    Ok(Some(status)) => {
                        let code = status.exit_code() as i32;
                        let _ = exit_tx.blocking_send(code);
                        break;
                    }
                    Ok(None) => {
                        // Still running
                    }
                    Err(e) => {
                        tracing::error!("PTY wait error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            input_tx,
            output_rx,
            exit_rx,
            _task_handle: task_handle,
        })
    }

    /// Send input to the PTY
    pub async fn send_input(&self, data: Vec<u8>) -> anyhow::Result<()> {
        self.input_tx
            .send(data)
            .await
            .map_err(|_| anyhow::anyhow!("PTY input channel closed"))?;
        Ok(())
    }

    /// Receive output from the PTY
    pub async fn recv_output(&mut self) -> Option<Vec<u8>> {
        self.output_rx.recv().await
    }

    /// Try to receive exit status (non-blocking)
    pub fn try_recv_exit(&mut self) -> Option<i32> {
        self.exit_rx.try_recv().ok()
    }

    /// Wait for exit status
    pub async fn wait_exit(&mut self) -> Option<i32> {
        self.exit_rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_config_default() {
        let config = PtyConfig::default();
        assert_eq!(config.term, "xterm-256color");
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
    }

    #[test]
    fn test_pty_size_conversion() {
        let config = PtyConfig {
            term: "xterm".to_string(),
            cols: 120,
            rows: 40,
            pixel_width: 0,
            pixel_height: 0,
        };
        let size: PtySize = config.into();
        assert_eq!(size.cols, 120);
        assert_eq!(size.rows, 40);
    }
}
