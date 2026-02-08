//! Ralph Mobile Server
//!
//! REST API + SSE server for mobile monitoring of Ralph orchestrator sessions.

mod api;
mod cli;
mod session;
mod watcher;

use actix_web::{App, HttpServer, middleware, web};
use api::{AppState, ProcessManager, RobotState};
use clap::Parser;
use cli::Args;
use ralph_core::skill_registry::SkillRegistry;
use ralph_core::SkillsConfig;
use session::discover_sessions;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use watcher::{EventWatcher, SessionBroadcast};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize tracing with sensible defaults for HTTP request logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,actix_web=info,actix_server=info".parse().unwrap()),
        )
        .init();

    // Discover sessions in current directory
    let cwd = env::current_dir()?;
    let sessions = discover_sessions(&cwd);
    info!("Discovered {} session(s)", sessions.len());
    for session in &sessions {
        info!(
            "  Session {}: {:?} (task: {:?})",
            &session.id[..8],
            session.path,
            session.task_name
        );
    }

    let bind_addr = args.bind_address();
    info!("Starting ralph-mobile-server on {}", bind_addr);

    // Create broadcast handles for each session's events file
    // Spawn background tasks to watch for file changes and broadcast events.
    let mut watchers: HashMap<String, SessionBroadcast> = HashMap::new();
    for session in &sessions {
        // Resolve the actual events file path (handles current-events pointer)
        if let Some(events_path) = session::resolve_events_path(&session.path) {
            match EventWatcher::new(&events_path) {
                Ok(mut watcher) => {
                    info!("Watching events for session {}", &session.id[..8]);
                    let broadcast = watcher.broadcast_handle();
                    watchers.insert(session.id.clone(), broadcast);

                    // Spawn background task to pump events from file watcher to broadcast channel
                    let session_id = session.id[..8].to_string();
                    actix_web::rt::spawn(async move {
                        loop {
                            // Block waiting for file changes, then broadcast
                            if watcher.wait_for_events(Duration::from_secs(60)).is_some() {
                                debug!("Broadcast events for session {}", session_id);
                            }
                        }
                    });
                }
                Err(e) => {
                    info!("Could not watch session {}: {}", &session.id[..8], e);
                }
            }
        } else {
            debug!("No events file found for session {}", &session.id[..8]);
        }
    }

    // Create application state with sessions, watchers, and active sessions registry
    let app_state = web::Data::new(AppState {
        sessions,
        watchers: Arc::new(RwLock::new(watchers)),
        active_sessions: Arc::new(RwLock::new(HashMap::new())),
    });

    // Create process manager for spawned sessions
    let process_manager = web::Data::new(ProcessManager::new());

    // Create robot state for human-in-the-loop tracking
    let robot_state = web::Data::new(RobotState::new());

    // Initialize skill registry with built-in skills
    let skills_config = SkillsConfig {
        enabled: true,
        dirs: vec![],
        overrides: HashMap::new(),
    };
    let skill_registry = match SkillRegistry::from_config(&skills_config, &cwd, None) {
        Ok(registry) => {
            info!("Loaded {} skill(s)", registry.skills_for_hat(None).len());
            web::Data::new(Arc::new(RwLock::new(registry)))
        }
        Err(e) => {
            tracing::warn!("Failed to initialize skill registry: {}", e);
            // Create empty registry as fallback (built-in skills only)
            web::Data::new(Arc::new(RwLock::new(SkillRegistry::new(None))))
        }
    };

    HttpServer::new(move || {
        App::new()
            // HTTP request logging middleware
            .wrap(middleware::Logger::default())
            // Shared application state
            .app_data(app_state.clone())
            .app_data(process_manager.clone())
            .app_data(robot_state.clone())
            .app_data(skill_registry.clone())
            // Health endpoint
            .route("/health", web::get().to(|| async { "OK" }))
            // API routes without authentication (local server only)
            .service(web::scope("/api").configure(api::configure))
    })
    .bind(&bind_addr)?
    .run()
    .await
}
