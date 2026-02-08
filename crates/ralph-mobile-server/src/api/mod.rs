//! API module for ralph-mobile-server.
//!
//! Contains REST endpoints and SSE streaming handlers.

mod config_export;
mod configs;
mod events;
mod hats;
mod health;
mod host;
mod iterations;
mod loops;
mod memories;
mod merge_queue;
mod presets;
mod prompts;
mod robot;
mod runner;
mod sessions;
mod skills;
mod tasks;

use actix_web::web;

pub use events::AppState;
pub use robot::RobotState;
pub use runner::ProcessManager;

/// Configure API routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // Health check
        .route("/health", web::get().to(health::health))
        // Sessions
        .route("/sessions", web::get().to(sessions::list_sessions))
        .route("/sessions", web::post().to(runner::start_session))
        .route("/sessions/{id}", web::delete().to(runner::stop_session))
        .route(
            "/sessions/{id}/status",
            web::get().to(sessions::get_session_status),
        )
        .route(
            "/sessions/{id}/events",
            web::get().to(events::stream_events),
        )
        .route(
            "/sessions/{id}/emit",
            web::post().to(events::emit_event),
        )
        .route(
            "/sessions/{id}/pause",
            web::post().to(runner::pause_session),
        )
        .route(
            "/sessions/{id}/resume",
            web::post().to(runner::resume_session),
        )
        .route(
            "/sessions/{id}/steer",
            web::post().to(runner::steer_session),
        )
        .route(
            "/sessions/{id}/scratchpad",
            web::get().to(sessions::get_scratchpad),
        )
        // Memories
        .route("/memories", web::get().to(memories::get_memories))
        .route("/memories", web::put().to(memories::update_memories))
        .route("/memories/export", web::post().to(memories::export_memories))
        // Configs
        .route("/configs", web::get().to(configs::list_configs))
        .route("/configs/{path:.*}", web::get().to(configs::get_config_content))
        // Config export/import
        .route("/config/export", web::post().to(config_export::export_config))
        .route("/config/import", web::post().to(config_export::import_config))
        // Prompts
        .route("/prompts", web::get().to(prompts::list_prompts))
        .route("/prompts/{path:.*}", web::get().to(prompts::get_prompt_content))
        // Hats
        .route("/hats", web::get().to(hats::list_hats))
        // Presets
        .route("/presets", web::get().to(presets::list_presets))
        // Loops
        .route("/loops", web::get().to(loops::list_loops))
        .route("/loops", web::post().to(loops::spawn_loop))
        .route("/loops/{id}", web::get().to(loops::get_loop))
        .route("/loops/{id}/merge", web::post().to(loops::merge_loop))
        .route("/loops/{id}/discard", web::post().to(loops::discard_loop))
        // Iterations
        .route(
            "/sessions/{id}/iterations",
            web::get().to(iterations::get_iterations),
        )
        // Merge Queue
        .route("/merge-queue", web::get().to(merge_queue::get_merge_queue))
        // Tasks
        .route("/tasks", web::get().to(tasks::list_tasks))
        .route("/tasks", web::post().to(tasks::create_task))
        .route("/tasks/{id}", web::put().to(tasks::update_task))
        // Host metrics
        .route("/host/metrics", web::get().to(host::get_metrics))
        .route("/host/processes", web::get().to(host::get_processes))
        // Human-in-the-Loop (RObot)
        .route("/robot/questions", web::get().to(robot::get_questions))
        .route("/robot/response", web::post().to(robot::post_response))
        .route("/robot/guidance", web::post().to(robot::post_guidance))
        // Skills
        .route("/skills", web::get().to(skills::list_skills))
        .route("/skills/{name}", web::get().to(skills::get_skill))
        .route("/skills/{name}/load", web::post().to(skills::load_skill));
}
