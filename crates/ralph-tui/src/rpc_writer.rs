//! RPC command writer for sending commands to a subprocess.
//!
//! This module provides functions to write `RpcCommand` objects as JSON lines
//! to a subprocess's stdin. It replaces the in-process guidance queue and
//! interrupt channel when running in subprocess mode.

use std::sync::Arc;

use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::debug;

use ralph_proto::json_rpc::RpcCommand;

/// Writer for sending RPC commands to a subprocess.
///
/// This struct wraps an async writer (typically `tokio::process::ChildStdin`)
/// and provides methods to send typed commands as JSON lines.
pub struct RpcWriter<W> {
    writer: Arc<Mutex<W>>,
}

impl<W: AsyncWrite + Unpin + Send> RpcWriter<W> {
    /// Creates a new RPC writer wrapping the given async writer.
    pub fn new(writer: W) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }

    /// Sends a guidance message for the next iteration.
    ///
    /// The subprocess will queue this guidance and inject it at the start
    /// of the next iteration.
    pub async fn send_guidance(&self, message: &str) -> std::io::Result<()> {
        let cmd = RpcCommand::Guidance {
            id: None,
            message: message.to_string(),
        };
        self.send_command(&cmd).await
    }

    /// Sends a steer message for immediate injection.
    ///
    /// The subprocess will attempt to inject this guidance into the
    /// currently running iteration.
    pub async fn send_steer(&self, message: &str) -> std::io::Result<()> {
        let cmd = RpcCommand::Steer {
            id: None,
            message: message.to_string(),
        };
        self.send_command(&cmd).await
    }

    /// Sends an abort command to terminate the loop.
    ///
    /// The subprocess will initiate graceful termination of the
    /// orchestration loop.
    pub async fn send_abort(&self) -> std::io::Result<()> {
        let cmd = RpcCommand::Abort {
            id: None,
            reason: Some("User requested abort".to_string()),
        };
        self.send_command(&cmd).await
    }

    /// Sends a follow-up message for the next iteration.
    pub async fn send_follow_up(&self, message: &str) -> std::io::Result<()> {
        let cmd = RpcCommand::FollowUp {
            id: None,
            message: message.to_string(),
        };
        self.send_command(&cmd).await
    }

    /// Sends a get_state command to request the current loop state.
    pub async fn send_get_state(&self, id: Option<String>) -> std::io::Result<()> {
        let cmd = RpcCommand::GetState { id };
        self.send_command(&cmd).await
    }

    /// Sends a set_hat command to change the hat for the next iteration.
    pub async fn send_set_hat(&self, hat: &str) -> std::io::Result<()> {
        let cmd = RpcCommand::SetHat {
            id: None,
            hat: hat.to_string(),
        };
        self.send_command(&cmd).await
    }

    /// Sends a raw RPC command.
    async fn send_command(&self, cmd: &RpcCommand) -> std::io::Result<()> {
        let json = serde_json::to_string(cmd)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        debug!(command = %json, "Sending RPC command");

        let line = format!("{}\n", json);
        let mut writer = self.writer.lock().await;
        writer.write_all(line.as_bytes()).await?;
        writer.flush().await?;

        Ok(())
    }

    /// Closes the writer, signaling EOF to the subprocess.
    pub async fn close(&self) -> std::io::Result<()> {
        let mut writer = self.writer.lock().await;
        writer.shutdown().await
    }
}

/// Creates a cloneable handle to the RPC writer.
impl<W> Clone for RpcWriter<W> {
    fn clone(&self) -> Self {
        Self {
            writer: Arc::clone(&self.writer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn test_send_guidance() {
        let (client, mut server) = tokio::io::duplex(1024);
        let writer = RpcWriter::new(client);

        writer.send_guidance("focus on tests").await.unwrap();

        let mut buf = vec![0u8; 1024];
        let n = server.read(&mut buf).await.unwrap();
        let line = std::str::from_utf8(&buf[..n]).unwrap();

        assert!(line.contains(r#""type":"guidance""#));
        assert!(line.contains(r#""message":"focus on tests""#));
    }

    #[tokio::test]
    async fn test_send_abort() {
        let (client, mut server) = tokio::io::duplex(1024);
        let writer = RpcWriter::new(client);

        writer.send_abort().await.unwrap();

        let mut buf = vec![0u8; 1024];
        let n = server.read(&mut buf).await.unwrap();
        let line = std::str::from_utf8(&buf[..n]).unwrap();

        assert!(line.contains(r#""type":"abort""#));
    }

    #[tokio::test]
    async fn test_send_steer() {
        let (client, mut server) = tokio::io::duplex(1024);
        let writer = RpcWriter::new(client);

        writer.send_steer("change approach").await.unwrap();

        let mut buf = vec![0u8; 1024];
        let n = server.read(&mut buf).await.unwrap();
        let line = std::str::from_utf8(&buf[..n]).unwrap();

        assert!(line.contains(r#""type":"steer""#));
        assert!(line.contains(r#""message":"change approach""#));
    }

    #[tokio::test]
    async fn test_send_get_state() {
        let (client, mut server) = tokio::io::duplex(1024);
        let writer = RpcWriter::new(client);

        writer
            .send_get_state(Some("req-1".to_string()))
            .await
            .unwrap();

        let mut buf = vec![0u8; 1024];
        let n = server.read(&mut buf).await.unwrap();
        let line = std::str::from_utf8(&buf[..n]).unwrap();

        assert!(line.contains(r#""type":"get_state""#));
        assert!(line.contains(r#""id":"req-1""#));
    }

    #[tokio::test]
    async fn test_writer_clone() {
        let (client, _server) = tokio::io::duplex(1024);
        let writer1 = RpcWriter::new(client);
        let writer2 = writer1.clone();

        // Both should work
        writer1.send_guidance("test1").await.unwrap();
        writer2.send_guidance("test2").await.unwrap();
    }
}
