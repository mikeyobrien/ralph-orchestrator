use std::future::Future;

use anyhow::{Context, Result};
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, info, warn};

use crate::config::ApiConfig;
use crate::runtime::RpcRuntime;
use crate::stream_domain::KEEPALIVE_INTERVAL_MS;

#[derive(Clone)]
struct AppState {
    runtime: RpcRuntime,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamQuery {
    subscription_id: Option<String>,
}

pub fn router(runtime: RpcRuntime) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/rpc/v1", post(rpc_handler))
        .route("/rpc/v1/capabilities", get(capabilities_handler))
        .route("/rpc/v1/stream", get(stream_handler))
        .with_state(AppState { runtime })
}

/// Format a host+port into a bind address string.
///
/// IPv6 addresses must be wrapped in brackets so the port is unambiguous:
/// `::1` → `[::1]:3000`. Already-bracketed hosts (e.g. `[::1]`) are left
/// as-is to prevent double-wrapping. IPv4 and hostnames are unchanged.
pub(crate) fn bind_addr_string(host: &str, port: u16) -> String {
    let trimmed = host.trim();
    if trimmed.starts_with('[') {
        // Already bracketed (e.g. "[::1]" from env var)
        format!("{trimmed}:{port}")
    } else if trimmed.contains(':') {
        // Raw IPv6 literal — add brackets
        format!("[{trimmed}]:{port}")
    } else {
        format!("{trimmed}:{port}")
    }
}

pub async fn serve(config: ApiConfig) -> Result<()> {
    let bind_address = bind_addr_string(&config.host, config.port);
    let listener = TcpListener::bind(&bind_address)
        .await
        .with_context(|| format!("failed binding listener at {bind_address}"))?;

    info!(
        address = %bind_address,
        auth_mode = %config.auth_mode.as_contract_mode(),
        "starting ralph-api server"
    );

    let runtime = RpcRuntime::new(config)?;
    serve_with_listener(listener, runtime, shutdown_signal()).await
}

pub async fn serve_with_listener<F>(
    listener: TcpListener,
    runtime: RpcRuntime,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let local_addr = listener
        .local_addr()
        .context("failed to read listener local_addr")?;
    info!(%local_addr, "ralph-api listening");

    axum::serve(listener, router(runtime))
        .with_graceful_shutdown(shutdown)
        .await
        .context("axum server terminated with error")
}

async fn health_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(state.runtime.health_payload())
}

async fn capabilities_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(state.runtime.capabilities_payload())
}

async fn rpc_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let (status, payload) = state.runtime.handle_http_request(&body, &headers);
    (status, Json(payload))
}

async fn stream_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<StreamQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let principal = match state.runtime.authenticate_websocket(&headers) {
        Ok(p) => p,
        Err(error) => {
            let status = error.status;
            let error_payload =
                crate::protocol::error_envelope(&error, &state.runtime.config.served_by);
            return (status, Json(error_payload)).into_response();
        }
    };

    ws.on_upgrade(move |socket| {
        stream_connection(socket, state.runtime, query.subscription_id, principal)
    })
}

async fn stream_connection(
    mut socket: WebSocket,
    runtime: RpcRuntime,
    subscription_id: Option<String>,
    principal: String,
) {
    let Some(subscription_id) = subscription_id else {
        warn!("stream connection missing subscriptionId query parameter");
        let _ = socket.close().await;
        return;
    };

    let streams = runtime.stream_domain();
    if !streams.has_subscription(&subscription_id) {
        warn!(subscription_id, "stream subscription does not exist");
        let _ = socket.close().await;
        return;
    }

    if streams
        .get_subscription_principal(&subscription_id)
        .as_deref()
        != Some(principal.as_str())
    {
        warn!(subscription_id, "stream connection auth principal mismatch");
        let _ = socket.close().await;
        return;
    }

    let replay = match streams.replay_for_subscription(&subscription_id) {
        Ok(replay) => replay,
        Err(error) => {
            warn!(subscription_id, error = %error.message, "failed preparing replay batch");
            let _ = socket.close().await;
            return;
        }
    };

    if replay.dropped_count > 0 {
        let event = streams.backpressure_event(&subscription_id, replay.dropped_count);
        if !send_stream_event(&mut socket, &event).await {
            return;
        }
    }

    for event in replay.events {
        if !send_stream_event(&mut socket, &event).await {
            return;
        }
    }

    let mut live_rx = streams.live_receiver();
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(KEEPALIVE_INTERVAL_MS));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let keepalive = streams.keepalive_event(&subscription_id, KEEPALIVE_INTERVAL_MS);
                if !send_stream_event(&mut socket, &keepalive).await {
                    break;
                }
            }
            message = live_rx.recv() => {
                match message {
                    Ok(event) => {
                        if streams.matches_subscription(&subscription_id, &event)
                            && !send_stream_event(&mut socket, &event).await
                        {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        let event = streams.backpressure_event(
                            &subscription_id,
                            usize::try_from(skipped).unwrap_or(usize::MAX),
                        );
                        if !send_stream_event(&mut socket, &event).await {
                            break;
                        }
                    }
                    Err(RecvError::Closed) => break,
                }
            }
            message = socket.next() => {
                match message {
                    None | Some(Ok(Message::Close(_)) | Err(_)) => break,
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(_) | Message::Binary(_) | Message::Pong(_))) => {}
                }
            }
        }
    }
}

async fn send_stream_event(
    socket: &mut WebSocket,
    event: &crate::stream_domain::StreamEventEnvelope,
) -> bool {
    match serde_json::to_string(event) {
        Ok(serialized) => socket.send(Message::Text(serialized.into())).await.is_ok(),
        Err(error) => {
            error!(%error, "failed to serialize stream event");
            false
        }
    }
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        error!(%error, "failed waiting for ctrl-c shutdown signal");
    }
    info!("shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::bind_addr_string;

    #[test]
    fn ipv4_loopback_formats_without_brackets() {
        assert_eq!(bind_addr_string("127.0.0.1", 3000), "127.0.0.1:3000");
    }

    #[test]
    fn hostname_formats_without_brackets() {
        assert_eq!(bind_addr_string("localhost", 8080), "localhost:8080");
    }

    #[test]
    fn ipv6_loopback_wraps_in_brackets() {
        assert_eq!(bind_addr_string("::1", 3000), "[::1]:3000");
    }

    #[test]
    fn ipv6_any_wraps_in_brackets() {
        assert_eq!(bind_addr_string("::", 3000), "[::]:3000");
    }

    #[test]
    fn ipv6_full_address_wraps_in_brackets() {
        assert_eq!(bind_addr_string("2001:db8::1", 443), "[2001:db8::1]:443");
    }

    #[test]
    fn pre_bracketed_ipv6_does_not_double_wrap() {
        // RALPH_API_HOST=[::1] should not become [[::1]]:3000
        assert_eq!(bind_addr_string("[::1]", 3000), "[::1]:3000");
    }
}
