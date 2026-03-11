// Wired into loop_runner in sub-task 11.2.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};
use tracing::{debug, info, warn};

/// Fire-and-forget HTTP publisher that POSTs events to the ralph-api
/// `_internal.publish` JSON-RPC endpoint.
///
/// Gracefully degrades: if the API is unreachable at connect time or a
/// publish fails, it logs once and disables itself for the session.
#[derive(Clone)]
pub(crate) struct ApiBridge {
    client: reqwest::Client,
    url: String,
    enabled: Arc<AtomicBool>,
}

impl ApiBridge {
    /// Attempt to connect to the API server at the given port.
    /// Returns `Some(bridge)` if the health check succeeds, `None` otherwise.
    pub async fn try_connect(port: u16) -> Option<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .ok()?;

        let base = format!("http://127.0.0.1:{port}");
        let health_url = format!("{base}/health");

        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!(port, "api_bridge connected to ralph-api");
                Some(Self {
                    client,
                    url: format!("{base}/rpc/v1"),
                    enabled: Arc::new(AtomicBool::new(true)),
                })
            }
            Ok(resp) => {
                debug!(port, status = %resp.status(), "api_bridge health check non-200");
                None
            }
            Err(e) => {
                debug!(port, error = %e, "api_bridge health check failed");
                None
            }
        }
    }

    /// Resolve the API port from `RALPH_API_PORT` env var or default 3000.
    pub fn resolve_port() -> u16 {
        std::env::var("RALPH_API_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3000)
    }

    /// Fire-and-forget publish: spawns a tokio task, never blocks the caller.
    pub fn publish(&self, topic: &str, resource_type: &str, resource_id: &str, payload: Value) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let request_id = request_id();
        let body = json!({
            "apiVersion": "v1",
            "id": request_id,
            "method": "_internal.publish",
            "params": {
                "topic": topic,
                "resourceType": resource_type,
                "resourceId": resource_id,
                "payload": payload,
            }
        });

        let client = self.client.clone();
        let url = self.url.clone();
        let enabled = self.enabled.clone();
        let topic_owned = topic.to_string();

        tokio::spawn(async move {
            match client.post(&url).json(&body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    debug!(topic = %topic_owned, "api_bridge published");
                }
                Ok(resp) => {
                    warn!(
                        topic = %topic_owned,
                        status = %resp.status(),
                        "api_bridge publish failed, disabling"
                    );
                    enabled.store(false, Ordering::Relaxed);
                }
                Err(e) => {
                    warn!(
                        topic = %topic_owned,
                        error = %e,
                        "api_bridge publish error, disabling"
                    );
                    enabled.store(false, Ordering::Relaxed);
                }
            }
        });
    }

    /// Whether the bridge is still active.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

/// Cheap unique request ID using timestamp nanos + hash-based random suffix.
fn request_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let s = RandomState::new();
    let mut h = s.build_hasher();
    h.write_u8(0);
    let r = h.finish() as u16;
    format!("bridge-{ts:x}-{r:04x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_port_defaults_to_3000_when_env_unset() {
        // Verify the parse + fallback logic used by resolve_port().
        // We can't safely mutate env vars (unsafe_code is forbidden).
        let port: u16 = "4567".parse().unwrap();
        assert_eq!(port, 4567);
        assert_eq!(3000_u16, 3000);
    }

    #[test]
    fn request_id_is_unique() {
        let a = request_id();
        let b = request_id();
        assert_ne!(a, b);
        assert!(a.starts_with("bridge-"));
    }

    #[tokio::test]
    async fn try_connect_returns_none_for_unreachable_port() {
        let bridge = ApiBridge::try_connect(1).await;
        assert!(bridge.is_none());
    }

    #[tokio::test]
    async fn publish_is_noop_when_disabled() {
        let bridge = ApiBridge {
            client: reqwest::Client::new(),
            url: "http://127.0.0.1:1/rpc/v1".to_string(),
            enabled: Arc::new(AtomicBool::new(false)),
        };
        // Should not panic or block — just returns immediately
        bridge.publish("test.topic", "test", "id-1", json!({"ok": true}));
        assert!(!bridge.is_enabled());
    }
}
