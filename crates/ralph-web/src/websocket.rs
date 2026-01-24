//! WebSocket infrastructure for real-time updates.
//!
//! Provides a WebSocket hub that broadcasts file watcher events to connected clients.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{RwLock, mpsc};

/// Maximum number of concurrent WebSocket connections.
pub const MAX_CONNECTIONS: usize = 10;

/// Unique identifier for a WebSocket connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(u64);

impl ConnectionId {
    /// Generate a new unique connection ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Messages sent from server to WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Agent output lines
    Output { lines: Vec<String> },
    /// New iteration started
    IterationStarted { iteration: u32, hat: String },
    /// Event published
    Event { topic: String, payload: String },
    /// Loop completed
    LoopCompleted { reason: String },
    /// Connection accepted
    Connected { connection_id: u64 },
    /// Error message
    Error { message: String },
}

/// Hub for managing WebSocket connections and broadcasting messages.
#[derive(Clone)]
pub struct WebSocketHub {
    connections: Arc<RwLock<HashMap<ConnectionId, mpsc::Sender<ServerMessage>>>>,
}

impl WebSocketHub {
    /// Create a new WebSocket hub.
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new connection to the hub.
    ///
    /// Returns `Err` if the maximum number of connections is reached.
    pub async fn add(
        &self,
        id: ConnectionId,
        tx: mpsc::Sender<ServerMessage>,
    ) -> Result<(), WebSocketError> {
        let mut connections = self.connections.write().await;

        if connections.len() >= MAX_CONNECTIONS {
            return Err(WebSocketError::ConnectionLimitReached);
        }

        connections.insert(id, tx);
        Ok(())
    }

    /// Remove a connection from the hub.
    pub async fn remove(&self, id: &ConnectionId) {
        let mut connections = self.connections.write().await;
        connections.remove(id);
    }

    /// Broadcast a message to all connected clients.
    pub async fn broadcast(&self, msg: ServerMessage) {
        let connections = self.connections.read().await;

        for (_, tx) in connections.iter() {
            // Ignore send errors - the connection cleanup will handle dead connections
            let _ = tx.send(msg.clone()).await;
        }
    }

    /// Get the current number of connections.
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
}

impl Default for WebSocketHub {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket errors.
#[derive(Debug, Clone, PartialEq)]
pub enum WebSocketError {
    /// Maximum connection limit reached
    ConnectionLimitReached,
}

impl std::fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSocketError::ConnectionLimitReached => {
                write!(f, "Maximum connection limit of {} reached", MAX_CONNECTIONS)
            }
        }
    }
}

impl std::error::Error for WebSocketError {}

// ==================== Axum WebSocket Handler ====================

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};

/// WebSocket route handler.
///
/// Upgrades HTTP connection to WebSocket and manages the connection lifecycle.
pub async fn ws_handler(ws: WebSocketUpgrade, State(hub): State<WebSocketHub>) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, hub))
}

/// Handle an individual WebSocket connection.
async fn handle_socket(socket: WebSocket, hub: WebSocketHub) {
    let (mut sender, mut receiver) = socket.split();

    // Create channel for this connection
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(32);
    let id = ConnectionId::new();

    // Try to add connection to hub
    if let Err(e) = hub.add(id, tx).await {
        // Send error and close
        let error_msg = ServerMessage::Error {
            message: e.to_string(),
        };
        if let Ok(json) = serde_json::to_string(&error_msg) {
            let _ = sender.send(Message::Text(json.into())).await;
        }
        return;
    }

    // Send connected message
    let connected = ServerMessage::Connected {
        connection_id: id.0,
    };
    if let Ok(json) = serde_json::to_string(&connected)
        && sender.send(Message::Text(json.into())).await.is_err()
    {
        hub.remove(&id).await;
        return;
    }

    // Spawn task to forward messages from hub to client
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg)
                && sender.send(Message::Text(json.into())).await.is_err()
            {
                break;
            }
        }
    });

    // Process incoming messages (for ping/pong and close)
    while let Some(result) = receiver.next().await {
        match result {
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(data)) => {
                // Pong is handled automatically by axum
                tracing::debug!("Received ping: {:?}", data);
            }
            Err(_) => break,
            _ => {}
        }
    }

    // Cleanup
    hub.remove(&id).await;
    send_task.abort();
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ServerMessage Tests ====================

    #[test]
    fn test_server_message_output_serializes() {
        let msg = ServerMessage::Output {
            lines: vec!["line 1".to_string(), "line 2".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn test_server_message_iteration_started_serializes() {
        let msg = ServerMessage::IterationStarted {
            iteration: 5,
            hat: "builder".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"iteration_started\""));
        assert!(json.contains("\"iteration\":5"));
        assert!(json.contains("\"hat\":\"builder\""));
    }

    #[test]
    fn test_server_message_event_serializes() {
        let msg = ServerMessage::Event {
            topic: "build.start".to_string(),
            payload: "{}".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    #[test]
    fn test_server_message_loop_completed_serializes() {
        let msg = ServerMessage::LoopCompleted {
            reason: "LOOP_COMPLETE".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }

    // ==================== ConnectionId Tests ====================

    #[test]
    fn test_connection_id_unique() {
        let id1 = ConnectionId::new();
        let id2 = ConnectionId::new();
        assert_ne!(id1, id2);
    }

    // ==================== WebSocketHub Tests ====================

    #[tokio::test]
    async fn test_websocket_hub_add_connection() {
        let hub = WebSocketHub::new();
        let (tx, _rx) = mpsc::channel(16);
        let id = ConnectionId::new();

        let result = hub.add(id, tx).await;
        assert!(result.is_ok());
        assert_eq!(hub.connection_count().await, 1);
    }

    #[tokio::test]
    async fn test_websocket_hub_remove_connection() {
        let hub = WebSocketHub::new();
        let (tx, _rx) = mpsc::channel(16);
        let id = ConnectionId::new();

        hub.add(id, tx).await.unwrap();
        assert_eq!(hub.connection_count().await, 1);

        hub.remove(&id).await;
        assert_eq!(hub.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_websocket_disconnect_cleanup() {
        let hub = WebSocketHub::new();
        let (tx, _rx) = mpsc::channel(16);
        let id = ConnectionId::new();

        hub.add(id, tx).await.unwrap();
        assert_eq!(hub.connection_count().await, 1);

        // Simulate disconnect by removing
        hub.remove(&id).await;
        assert_eq!(hub.connection_count().await, 0);

        // Verify we can add new connections after cleanup
        let (tx2, _rx2) = mpsc::channel(16);
        let id2 = ConnectionId::new();
        hub.add(id2, tx2).await.unwrap();
        assert_eq!(hub.connection_count().await, 1);
    }

    #[tokio::test]
    async fn test_websocket_broadcast_to_multiple() {
        let hub = WebSocketHub::new();

        // Add two connections
        let (tx1, mut rx1) = mpsc::channel(16);
        let id1 = ConnectionId::new();
        hub.add(id1, tx1).await.unwrap();

        let (tx2, mut rx2) = mpsc::channel(16);
        let id2 = ConnectionId::new();
        hub.add(id2, tx2).await.unwrap();

        // Broadcast a message
        let msg = ServerMessage::IterationStarted {
            iteration: 1,
            hat: "ralph".to_string(),
        };
        hub.broadcast(msg.clone()).await;

        // Both should receive it
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert_eq!(received1, msg);
        assert_eq!(received2, msg);
    }

    #[tokio::test]
    async fn test_websocket_connection_limit() {
        let hub = WebSocketHub::new();

        // Add MAX_CONNECTIONS
        for _ in 0..MAX_CONNECTIONS {
            let (tx, _rx) = mpsc::channel(16);
            let id = ConnectionId::new();
            hub.add(id, tx).await.unwrap();
        }

        assert_eq!(hub.connection_count().await, MAX_CONNECTIONS);

        // Try to add one more - should fail
        let (tx, _rx) = mpsc::channel(16);
        let id = ConnectionId::new();
        let result = hub.add(id, tx).await;

        assert!(matches!(
            result,
            Err(WebSocketError::ConnectionLimitReached)
        ));
    }

    #[tokio::test]
    async fn test_websocket_connect_and_receive() {
        let hub = WebSocketHub::new();

        // Add a connection
        let (tx, mut rx) = mpsc::channel(16);
        let id = ConnectionId::new();
        hub.add(id, tx).await.unwrap();

        // Send a connected message
        hub.broadcast(ServerMessage::Connected {
            connection_id: id.0,
        })
        .await;

        // Verify received
        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, ServerMessage::Connected { .. }));

        // Send output
        hub.broadcast(ServerMessage::Output {
            lines: vec!["Hello".to_string()],
        })
        .await;

        let msg = rx.recv().await.unwrap();
        assert!(
            matches!(msg, ServerMessage::Output { lines } if lines == vec!["Hello".to_string()])
        );
    }
}
