//! `ralph-chat-mock` — a stand-in for the icedrop `POST /commands` and `GET /events` routes.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use ralph_chat_mock::{AppState, MockConfig, serve};
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(
    name = "ralph-chat-mock",
    about = "Mock icedrop POST /commands and GET /events for the v3 chat test page"
)]
struct Args {
    /// Address to bind. The page defaults to the same origin, or `?daemon=` for another port.
    #[arg(long, default_value = "127.0.0.1:8788")]
    bind: SocketAddr,
    /// Directory from `trunk build` (serves the wasm page at `/`).
    #[arg(long)]
    dist: Option<PathBuf>,
    /// Pause between streamed text chunks, in milliseconds.
    #[arg(long, default_value_t = 50)]
    delay_ms: u64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let state = AppState::new(MockConfig {
        chunk_delay: Duration::from_millis(args.delay_ms),
        dist: args.dist,
    });
    let listener = TcpListener::bind(args.bind).await.expect("bind");
    let bound = listener.local_addr().expect("local addr");
    eprintln!("ralph-chat-mock is a mock, not the icedrop daemon");
    eprintln!("serving http://{bound}");
    serve(listener, state).await.expect("serve mock");
}
