//! Bot setup and management commands.
//!
//! Provides:
//! - `ralph bot onboard` — Interactive wizard for Telegram bot setup
//! - `ralph bot status` — Check current bot configuration status
//! - `ralph bot test` — Send a test message to verify the bot works
//! - `ralph bot token set <token>` — Store/overwrite the bot token

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use clap::{Parser, Subcommand};
use ralph_core::RalphConfig;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::{ConfigSource, HatsSource, default_config_path};

// ─────────────────────────────────────────────────────────────────────────────
// CLI STRUCTS
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
pub struct BotArgs {
    #[command(subcommand)]
    pub command: BotCommands,
}

#[derive(Subcommand, Debug)]
pub enum BotCommands {
    /// Interactive setup wizard for Telegram bot
    Onboard(OnboardArgs),
    /// Check current bot configuration status
    Status(StatusArgs),
    /// Send a test message to verify the bot works
    Test(TestArgs),
    /// Manage bot tokens
    Token(TokenArgs),
    /// Run as a persistent daemon, listening on Telegram and starting loops on demand
    Daemon(DaemonArgs),
    /// List or prune archived Slack thread bindings
    SlackArchives(SlackArchivesArgs),
}

#[derive(Parser, Debug)]
pub struct OnboardArgs {
    /// Configure Slack instead of Telegram
    #[arg(long)]
    pub slack: bool,

    /// Skip interactive token prompt, provide token directly
    #[arg(long)]
    pub token: Option<String>,

    /// Slack app-level token for Socket Mode (xapp-...); never printed
    #[arg(long)]
    pub app_token: Option<String>,

    /// Allowed Slack channel ID (repeatable)
    #[arg(long = "channel")]
    pub channel_ids: Vec<String>,

    /// Allowed Slack user ID (repeatable)
    #[arg(long = "user")]
    pub allowed_users: Vec<String>,

    /// Skip chat_id detection, provide chat_id directly
    #[arg(long)]
    pub chat_id: Option<i64>,

    /// Timeout in seconds for waiting for a Telegram message
    #[arg(long, default_value = "120")]
    pub timeout: u64,
}

#[derive(Parser, Debug)]
pub struct StatusArgs {
    /// Check Slack bot configuration instead of Telegram
    #[arg(long)]
    pub slack: bool,
}

#[derive(Parser, Debug)]
pub struct TestArgs {
    /// Send through Slack instead of Telegram
    #[arg(long)]
    pub slack: bool,

    /// Slack channel ID for --slack test messages
    #[arg(long)]
    pub channel: Option<String>,

    /// Message to send (default: "Hello from Ralph!")
    #[arg(default_value = "Hello from Ralph!")]
    pub message: String,
}

#[derive(Parser, Debug)]
pub struct TokenArgs {
    #[command(subcommand)]
    pub command: TokenCommands,
}

#[derive(Subcommand, Debug)]
pub enum TokenCommands {
    /// Store or overwrite the bot token
    Set(SetTokenArgs),
}

#[derive(Parser, Debug)]
pub struct SetTokenArgs {
    /// Telegram bot token to store
    #[arg(value_name = "TOKEN")]
    pub token: String,

    /// Optional config file to update with the token
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct DaemonArgs {
    /// Run Slack Socket Mode daemon instead of Telegram daemon
    #[arg(long)]
    pub slack: bool,
}

#[derive(Parser, Debug)]
pub struct SlackArchivesArgs {
    #[command(subcommand)]
    pub command: SlackArchiveCommands,
}

#[derive(Subcommand, Debug)]
pub enum SlackArchiveCommands {
    /// List completed/failed/stopped Slack loop thread bindings
    List,
    /// Prune local archived binding records, worktrees, and logs older than N days
    Prune {
        /// Retention window in days
        #[arg(long, default_value_t = 30)]
        older_than_days: i64,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// DISPATCHER
// ─────────────────────────────────────────────────────────────────────────────

pub async fn execute(
    args: BotArgs,
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    use_colors: bool,
) -> Result<()> {
    match args.command {
        BotCommands::Onboard(onboard_args) if onboard_args.slack => {
            onboard_slack(onboard_args, use_colors).await
        }
        BotCommands::Onboard(onboard_args) => onboard_telegram(onboard_args, use_colors).await,
        BotCommands::Status(status_args) if status_args.slack => {
            bot_status_slack(config_sources, use_colors)
        }
        BotCommands::Status(_) => bot_status(use_colors).await,
        BotCommands::Test(test_args) => bot_test(test_args, config_sources, use_colors).await,
        BotCommands::Token(token_args) => bot_token(token_args, use_colors),
        BotCommands::Daemon(daemon_args) => {
            run_daemon(daemon_args, config_sources, hats_source, use_colors).await
        }
        BotCommands::SlackArchives(archives_args) => slack_archives(archives_args, use_colors),
    }
}

fn slack_archives(args: SlackArchivesArgs, _use_colors: bool) -> Result<()> {
    let workspace_root = std::env::current_dir().context("Failed to resolve current directory")?;
    let manager =
        ralph_slack::SlackStateManager::new(workspace_root.join(".ralph/slack-state.json"));
    match args.command {
        SlackArchiveCommands::List => {
            let bindings = manager.archived_threads()?;
            if bindings.is_empty() {
                println!("No archived Slack thread bindings found.");
                return Ok(());
            }
            println!("Archived Slack thread bindings:");
            for binding in bindings {
                println!(
                    "{}	{:?}	channel={}	thread={}	repo={}	final_card={}	parent={}",
                    binding.loop_id,
                    binding.status,
                    binding.channel_id,
                    binding.thread_ts,
                    binding.workspace_root.display(),
                    binding.final_card_ts.as_deref().unwrap_or("none"),
                    binding.parent_loop_id.as_deref().unwrap_or("none")
                );
            }
        }
        SlackArchiveCommands::Prune { older_than_days } => {
            if older_than_days < 0 {
                anyhow::bail!("--older-than-days must be non-negative");
            }
            let cutoff = Utc::now() - Duration::days(older_than_days);
            let pruned = manager.prune_archived_older_than(cutoff)?;
            println!(
                "Pruned {} archived Slack binding(s) older than {} day(s). Slack history was not mutated.",
                pruned.len(),
                older_than_days
            );
            for loop_id in pruned {
                println!("  {loop_id}");
            }
        }
    }
    Ok(())
}

fn bot_token(args: TokenArgs, use_colors: bool) -> Result<()> {
    match args.command {
        TokenCommands::Set(set_args) => bot_token_set(set_args, use_colors),
    }
}

fn bot_token_set(args: SetTokenArgs, use_colors: bool) -> Result<()> {
    let token = args.token;
    let mut keychain_ok = false;

    match store_bot_token(&token) {
        Ok(()) => {
            keychain_ok = true;
            print_success(
                use_colors,
                "Token stored in OS keychain (ralph/telegram-bot-token)",
            );
        }
        Err(e) => {
            print_warning(
                use_colors,
                &format!("Could not store token in keychain: {e}"),
            );
        }
    }

    let has_config = args.config.is_some();
    let config_path = args.config.unwrap_or_else(|| PathBuf::from("ralph.yml"));

    let should_write_config = has_config || !keychain_ok;
    if should_write_config {
        save_bot_token_config(&config_path, &token)?;
        print_success(
            use_colors,
            &format!("Token stored in {}", config_path.display()),
        );
    }

    if !keychain_ok && !has_config {
        print_warning(
            use_colors,
            "Keychain storage failed; token saved to ralph.yml instead.",
        );
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// ONBOARD WIZARD
// ─────────────────────────────────────────────────────────────────────────────

async fn onboard_telegram(args: OnboardArgs, use_colors: bool) -> Result<()> {
    println!();
    if use_colors {
        println!("\x1b[1mRalph Telegram Bot Setup\x1b[0m");
        println!("\x1b[1m========================\x1b[0m");
    } else {
        println!("Ralph Telegram Bot Setup");
        println!("========================");
    }
    println!();

    // Step 1: Get token
    let token = if let Some(t) = args.token {
        t
    } else {
        println!("Step 1: Create a Telegram bot");
        println!("  1. Open Telegram and message @BotFather");
        println!("  2. Send /newbot and follow the prompts");
        println!("  3. Copy the bot token");
        println!();
        prompt_token()?
    };

    // Step 2: Validate token
    println!();
    println!("Step 2: Validate token");
    print!("  Checking token with Telegram API...");
    io::stdout().flush()?;

    let bot_info = match telegram_get_me(&token).await {
        Ok(info) => {
            println!();
            print_success(use_colors, &format!("Token valid! Bot: @{}", info.username));
            info
        }
        Err(e) => {
            println!();
            print_error(use_colors, &format!("Token validation failed: {e}"));
            println!();
            println!("  Troubleshooting:");
            println!("    - Check the token was copied correctly from BotFather");
            println!("    - Ensure the token hasn't been revoked");
            println!("    - Check your internet connection");
            anyhow::bail!("Token validation failed");
        }
    };

    // Step 3: Get chat_id
    let chat_id = if let Some(id) = args.chat_id {
        id
    } else {
        println!();
        println!("Step 3: Connect your Telegram account");
        println!(
            "  Send any message to your bot: https://t.me/{}",
            bot_info.username
        );
        print!("  Waiting for message... (timeout: {}s)", args.timeout);
        io::stdout().flush()?;

        match telegram_get_updates(&token, args.timeout).await {
            Ok(update) => {
                println!();
                print_success(
                    use_colors,
                    &format!(
                        "Message received from: {} (chat_id: {})",
                        update.from_name, update.chat_id
                    ),
                );
                update.chat_id
            }
            Err(e) => {
                println!();
                print_error(use_colors, &format!("No message received: {e}"));
                println!();
                println!("  Troubleshooting:");
                println!("    - Make sure you're messaging @{}", bot_info.username);
                println!("    - Try sending /start to the bot");
                println!(
                    "    - You can retry with: ralph bot onboard --token <token> --timeout 300"
                );
                anyhow::bail!("Chat ID detection failed");
            }
        }
    };

    // Step 4: Save configuration
    println!();
    println!("Step 4: Save configuration");

    // Store token in keychain (fallback to config if unavailable)
    let mut config_token: Option<&str> = None;
    match store_bot_token(&token) {
        Ok(()) => {
            print_success(
                use_colors,
                "Token stored in OS keychain (ralph/telegram-bot-token)",
            );
        }
        Err(e) => {
            print_warning(
                use_colors,
                &format!("Could not store token in keychain: {e}"),
            );
            println!("    Set RALPH_TELEGRAM_BOT_TOKEN env var instead.");
            config_token = Some(token.as_str());
        }
    }

    // Update ralph.yml
    match save_robot_config(args.timeout, config_token) {
        Ok(()) => {
            if config_token.is_some() {
                print_warning(
                    use_colors,
                    "Stored bot token in ralph.yml (legacy). Consider using env var or keychain.",
                );
            }
            print_success(use_colors, "Updated ralph.yml (RObot.enabled: true)");
        }
        Err(e) => {
            print_warning(use_colors, &format!("Could not update ralph.yml: {e}"));
            println!("    Add manually:");
            println!("      RObot:");
            println!("        enabled: true");
            println!("        timeout_seconds: {}", args.timeout);
        }
    }

    // Save telegram state
    match save_telegram_state(chat_id) {
        Ok(()) => {
            print_success(
                use_colors,
                &format!("Created .ralph/telegram-state.json (chat_id: {})", chat_id),
            );
        }
        Err(e) => {
            print_warning(use_colors, &format!("Could not save telegram state: {e}"));
        }
    }

    // Step 5: Verify
    println!();
    println!("Step 5: Verify");

    match telegram_send_message(
        &token,
        chat_id,
        "Ralph bot setup complete! I'm ready to assist during orchestration runs.",
    )
    .await
    {
        Ok(_) => {
            print_success(use_colors, "Test message sent to your Telegram!");
        }
        Err(e) => {
            print_warning(use_colors, &format!("Could not send test message: {e}"));
            println!("    Setup saved. Verify later with: ralph bot test");
        }
    }

    println!();
    if use_colors {
        println!(
            "\x1b[32mSetup complete!\x1b[0m Run `ralph run` to start with Telegram integration."
        );
    } else {
        println!("Setup complete! Run `ralph run` to start with Telegram integration.");
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// STATUS COMMAND
// ─────────────────────────────────────────────────────────────────────────────

async fn bot_status(use_colors: bool) -> Result<()> {
    println!();
    if use_colors {
        println!("\x1b[1mRalph Bot Status\x1b[0m");
        println!("\x1b[1m================\x1b[0m");
    } else {
        println!("Ralph Bot Status");
        println!("================");
    }
    println!();

    // Check keychain
    let keychain_token = load_bot_token();
    let has_keychain = keychain_token.is_some();
    if has_keychain {
        print_success(use_colors, "Keychain: token stored");
    } else {
        print_status(use_colors, "Keychain: no token found");
    }

    // Check env var
    let has_env = std::env::var("RALPH_TELEGRAM_BOT_TOKEN").is_ok();
    if has_env {
        print_success(use_colors, "Env var: RALPH_TELEGRAM_BOT_TOKEN set");
    } else {
        print_status(use_colors, "Env var: RALPH_TELEGRAM_BOT_TOKEN not set");
    }

    // Check config
    let config_token = load_config_bot_token();
    if config_token.is_some() {
        print_warning(
            use_colors,
            "Config: bot_token in ralph.yml (consider migrating to keychain)",
        );
    } else {
        print_status(use_colors, "Config: no token in ralph.yml");
    }

    // Check RObot enabled
    let robot_enabled = is_robot_enabled();
    if robot_enabled {
        print_success(use_colors, "RObot: enabled in ralph.yml");
    } else {
        print_status(use_colors, "RObot: not enabled in ralph.yml");
    }

    // Check telegram state
    let state_path = Path::new(".ralph/telegram-state.json");
    if state_path.exists() {
        if let Ok(content) = std::fs::read_to_string(state_path) {
            if let Ok(state) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(chat_id) = state.get("chat_id").and_then(|v| v.as_i64()) {
                    print_success(
                        use_colors,
                        &format!("Telegram state: chat_id = {}", chat_id),
                    );
                } else {
                    print_warning(use_colors, "Telegram state: file exists but no chat_id");
                }
            } else {
                print_warning(use_colors, "Telegram state: file exists but invalid JSON");
            }
        }
    } else {
        print_status(use_colors, "Telegram state: not found");
    }

    // Validate token if available
    let effective_token = std::env::var("RALPH_TELEGRAM_BOT_TOKEN")
        .ok()
        .or(keychain_token)
        .or(config_token);

    println!();
    if let Some(token) = effective_token {
        print!("  Validating token with Telegram API...");
        io::stdout().flush()?;
        match telegram_get_me(&token).await {
            Ok(info) => {
                println!();
                print_success(
                    use_colors,
                    &format!("Bot: @{} ({})", info.username, info.first_name),
                );
            }
            Err(e) => {
                println!();
                print_error(use_colors, &format!("Token validation failed: {e}"));
            }
        }
    } else {
        print_error(
            use_colors,
            "No token available. Run `ralph bot onboard` to set up.",
        );
    }

    Ok(())
}

async fn onboard_slack(args: OnboardArgs, use_colors: bool) -> Result<()> {
    println!();
    if use_colors {
        println!("\x1b[1mRalph Slack Bot Setup\x1b[0m");
        println!("\x1b[1m=====================\x1b[0m");
    } else {
        println!("Ralph Slack Bot Setup");
        println!("=====================");
    }
    println!(
        "Required Slack scopes: chat:write, files:write, app_mentions:read, commands (for slash commands), and channel history for thread replies."
    );
    println!("Socket Mode requires an xapp-... app token for `ralph bot daemon --slack`.");

    save_slack_robot_config(
        args.timeout,
        args.token.as_deref(),
        args.app_token.as_deref(),
        &args.channel_ids,
        &args.allowed_users,
    )?;
    print_success(use_colors, "Slack RObot config written to ralph.yml");
    if args.token.is_none() {
        print_status(
            use_colors,
            "Set RALPH_SLACK_BOT_TOKEN or add RObot.slack.bot_token before running Slack commands",
        );
    }
    if args.app_token.is_none() {
        print_status(
            use_colors,
            "Set RALPH_SLACK_APP_TOKEN or add RObot.slack.app_token before running daemon --slack",
        );
    }
    Ok(())
}

fn bot_status_slack(config_sources: &[ConfigSource], use_colors: bool) -> Result<()> {
    println!();
    if use_colors {
        println!("\x1b[1mRalph Slack Bot Status\x1b[0m");
        println!("\x1b[1m======================\x1b[0m");
    } else {
        println!("Ralph Slack Bot Status");
        println!("======================");
    }

    if std::env::var("RALPH_SLACK_BOT_TOKEN").is_ok() {
        print_success(use_colors, "Env var: RALPH_SLACK_BOT_TOKEN set");
    } else {
        print_status(use_colors, "Env var: RALPH_SLACK_BOT_TOKEN not set");
    }
    if std::env::var("RALPH_SLACK_APP_TOKEN").is_ok() {
        print_success(use_colors, "Env var: RALPH_SLACK_APP_TOKEN set");
    } else {
        print_status(use_colors, "Env var: RALPH_SLACK_APP_TOKEN not set");
    }

    let config_path = slack_config_path_from_sources(config_sources);
    if let Some(config) = load_slack_config_from(&config_path) {
        print_success(
            use_colors,
            &format!("Config: RObot.slack present in {}", config_path.display()),
        );
        if !config.channel_ids.is_empty() {
            print_success(
                use_colors,
                &format!("Allowed channels: {}", config.channel_ids.join(", ")),
            );
        }
        if !config.allowed_users.is_empty() {
            print_success(
                use_colors,
                &format!("Allowed users: {}", config.allowed_users.join(", ")),
            );
        }
    } else {
        print_status(
            use_colors,
            &format!("Config: no RObot.slack in {}", config_path.display()),
        );
    }

    let state_path = Path::new(".ralph/slack-state.json");
    if state_path.exists() {
        let state = ralph_slack::SlackStateManager::new(state_path).load_or_default()?;
        print_success(
            use_colors,
            &format!("Slack state: {} bound thread(s)", state.threads.len()),
        );
    } else {
        print_status(use_colors, "Slack state: not found");
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// TEST COMMAND
// ─────────────────────────────────────────────────────────────────────────────

async fn bot_test(args: TestArgs, config_sources: &[ConfigSource], use_colors: bool) -> Result<()> {
    if args.slack {
        return bot_test_slack(args, config_sources, use_colors).await;
    }

    // Resolve token
    let token = resolve_token().context(
        "No bot token available. Run `ralph bot onboard` or set RALPH_TELEGRAM_BOT_TOKEN",
    )?;

    // Resolve chat_id
    let chat_id =
        resolve_chat_id().context("No chat_id found. Run `ralph bot onboard` to detect it")?;

    print!("  Sending message to chat {}...", chat_id);
    io::stdout().flush()?;

    match telegram_send_message(&token, chat_id, &args.message).await {
        Ok(_) => {
            println!();
            print_success(use_colors, "Message sent!");
        }
        Err(e) => {
            println!();
            print_error(use_colors, &format!("Failed to send message: {e}"));
            anyhow::bail!("Send failed");
        }
    }

    Ok(())
}

async fn bot_test_slack(
    args: TestArgs,
    config_sources: &[ConfigSource],
    use_colors: bool,
) -> Result<()> {
    let config_path = slack_config_path_from_sources(config_sources);
    let token = std::env::var("RALPH_SLACK_BOT_TOKEN")
        .ok()
        .or_else(|| load_slack_config_from(&config_path).and_then(|config| config.bot_token))
        .context("No Slack bot token available. Set RALPH_SLACK_BOT_TOKEN or run `ralph bot onboard --slack --token <token>`")?;
    let channel = args
        .channel
        .or_else(|| {
            load_slack_config_from(&config_path)
                .and_then(|config| config.channel_ids.into_iter().next())
        })
        .context(
            "No Slack channel available. Pass --channel C... or configure RObot.slack.channel_ids",
        )?;

    print!("  Sending Slack message to channel {}...", channel);
    io::stdout().flush()?;
    let api = ralph_slack::SlackApi::new(token, None);
    api.post_message(&channel, None, &args.message).await?;
    println!();
    print_success(use_colors, "Slack message sent!");
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// DAEMON COMMAND
// ─────────────────────────────────────────────────────────────────────────────

/// Run the bot daemon — delegates to the configured communication adapter.
///
/// Telegram and Slack are supported. The adapter implements
/// [`DaemonAdapter`] or the Slack daemon seam and handles platform-specific concerns.
async fn run_daemon(
    args: DaemonArgs,
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    use_colors: bool,
) -> Result<()> {
    use ralph_proto::DaemonAdapter;

    let workspace_root = std::env::current_dir().context("Failed to get current directory")?;
    let primary_sources: Vec<_> = config_sources
        .iter()
        .filter(|s| !matches!(s, ConfigSource::Override { .. }))
        .collect();

    if primary_sources.len() > 1 {
        warn!("Multiple config sources specified, using first one. Others ignored.");
    }

    let has_overrides = config_sources
        .iter()
        .any(|s| matches!(s, ConfigSource::Override { .. }));
    if has_overrides || hats_source.is_some() {
        warn!("Config overrides/hats will be resolved into a temporary runtime config.");
    }

    let direct_file = if let Some(ConfigSource::File(path)) = primary_sources.first() {
        let path = if path.is_absolute() {
            path.clone()
        } else {
            workspace_root.join(path)
        };

        if !path.exists() {
            anyhow::bail!("Config file not found: {}", path.display());
        }

        if has_overrides || hats_source.is_some() {
            None
        } else {
            let config = RalphConfig::from_file(&path)
                .with_context(|| format!("Failed to load config from {}", path.display()))?;

            Some((config, path))
        }
    } else {
        None
    };

    let used_direct_file = direct_file.is_some();

    let (config, config_path) = if let Some((config, path)) = direct_file {
        (config, path)
    } else {
        let config = crate::preflight::load_config_for_preflight(config_sources, hats_source)
            .await
            .context("Failed to load config for bot daemon")?;
        let path = write_temp_config_for_daemon(&workspace_root, &config)
            .context("Failed to write temporary runtime config")?;
        (config, path)
    };

    // Preserve previous behavior for plain default run.
    let default_path = workspace_root.join("ralph.yml");
    if primary_sources.is_empty() && !has_overrides && !default_path.exists() {
        anyhow::bail!("Config file not found: {}", default_path.display());
    }

    if !primary_sources.is_empty() && !used_direct_file {
        warn!("Using resolved runtime config: {}", config_path.display());
    }

    let use_slack =
        args.slack || matches!(config.robot.surface(), Ok(ralph_core::RobotSurface::Slack));
    if use_slack {
        return run_slack_daemon(config, config_path, workspace_root, use_colors).await;
    }

    // Resolve bot token and chat_id for Telegram adapter
    let token = config.robot.resolve_bot_token().context(
        "No bot token available. Run `ralph bot onboard` or set RALPH_TELEGRAM_BOT_TOKEN",
    )?;
    let chat_id =
        resolve_chat_id().context("No chat_id found. Run `ralph bot onboard` to detect it")?;

    if use_colors {
        println!("\x1b[1mRalph Daemon\x1b[0m (Telegram)");
    } else {
        println!("Ralph Daemon (Telegram)");
    }

    // Resolve custom API URL (env var > config file)
    let api_url = std::env::var("RALPH_TELEGRAM_API_URL")
        .ok()
        .or_else(|| load_config_api_url_from(&config_path));

    // Build the adapter
    let adapter = ralph_telegram::TelegramDaemon::new(token, api_url, chat_id);

    // Build the start_loop callback — wraps our CLI loop runner
    let start_loop: ralph_proto::StartLoopFn = Box::new(move |prompt: String| {
        let config_path = Some(config_path.clone());
        Box::pin(async move {
            let ws = std::env::current_dir()?;
            let reason = crate::loop_runner::start_loop(prompt, ws, config_path).await?;
            Ok(format!("{:?}", reason))
        })
    });

    adapter.run_daemon(workspace_root, start_loop).await?;

    Ok(())
}

async fn run_slack_daemon(
    config: RalphConfig,
    config_path: PathBuf,
    workspace_root: PathBuf,
    use_colors: bool,
) -> Result<()> {
    let bot_token = config.robot.resolve_slack_bot_token().context(
        "No Slack bot token available. Set RALPH_SLACK_BOT_TOKEN or configure RObot.slack.bot_token",
    )?;
    let app_token = config.robot.resolve_slack_app_token().context(
        "No Slack app token available. Set RALPH_SLACK_APP_TOKEN or configure RObot.slack.app_token",
    )?;
    let slack = config.robot.slack.clone().unwrap_or_default();
    let (repo_aliases, channel_repos) = resolve_slack_repo_routing(&slack)?;

    if use_colors {
        println!("\x1b[1mRalph Daemon\x1b[0m (Slack Socket Mode)");
    } else {
        println!("Ralph Daemon (Slack Socket Mode)");
    }

    let api = ralph_slack::SlackApi::new(bot_token, None);
    let socket_url = api.open_socket_mode_url(&app_token).await?;
    let state = ralph_slack::SlackStateManager::new(workspace_root.join(".ralph/slack-state.json"));
    let daemon = ralph_slack::SlackDaemon::new(
        ralph_slack::SlackDaemonConfig {
            workspace_root: workspace_root.clone(),
            allowed_channels: slack.channel_ids,
            allowed_users: slack.allowed_users,
            repo_aliases,
            channel_repos,
        },
        state,
        ralph_slack::CommandLoopSpawner::new(Some(config_path)),
        ralph_slack::SlackApiNotifier::new(api),
    );
    ralph_slack::socket_mode::run_socket_mode(&socket_url, daemon).await?;
    Ok(())
}

fn resolve_slack_repo_routing(
    slack: &ralph_core::SlackBotConfig,
) -> Result<(BTreeMap<String, PathBuf>, BTreeMap<String, String>)> {
    if slack.channel_ids.is_empty()
        || slack.allowed_users.is_empty()
        || slack.channel_repos.is_empty()
    {
        anyhow::bail!(
            "Slack daemon requires explicit RObot.slack.channel_ids, RObot.slack.allowed_users, and RObot.slack.channel_repos"
        );
    }

    let mut repo_aliases = BTreeMap::new();
    for (alias, repo_root) in &slack.repo_aliases {
        if !is_safe_repo_alias(alias) {
            anyhow::bail!(
                "Slack repo alias {alias} is invalid; use only letters, numbers, hyphen, or underscore"
            );
        }
        if !repo_root.is_absolute() {
            anyhow::bail!(
                "Slack repo alias {alias} must be an absolute path: {}",
                repo_root.display()
            );
        }
        let canonical = repo_root.canonicalize().with_context(|| {
            format!(
                "Slack repo alias {alias} does not exist: {}",
                repo_root.display()
            )
        })?;
        repo_aliases.insert(alias.clone(), canonical);
    }

    let mut channel_repos = BTreeMap::new();
    for channel_id in &slack.channel_ids {
        let Some(selector) = slack.channel_repos.get(channel_id) else {
            anyhow::bail!(
                "Slack channel {channel_id} is missing RObot.slack.channel_repos mapping"
            );
        };
        if repo_aliases.contains_key(selector) {
            channel_repos.insert(channel_id.clone(), selector.clone());
            continue;
        }

        let legacy_repo_root = PathBuf::from(selector);
        if !legacy_repo_root.is_absolute() {
            anyhow::bail!("Slack channel {channel_id} references unknown repo alias {selector}");
        }
        let canonical = legacy_repo_root.canonicalize().with_context(|| {
            format!(
                "Slack legacy repo root for channel {channel_id} does not exist: {}",
                legacy_repo_root.display()
            )
        })?;
        let alias = insert_legacy_repo_alias(&mut repo_aliases, channel_id, canonical);
        channel_repos.insert(channel_id.clone(), alias);
    }

    Ok((repo_aliases, channel_repos))
}

fn insert_legacy_repo_alias(
    repo_aliases: &mut BTreeMap<String, PathBuf>,
    channel_id: &str,
    canonical_root: PathBuf,
) -> String {
    if let Some((alias, _)) = repo_aliases
        .iter()
        .find(|(_, root)| root.as_path() == canonical_root.as_path())
    {
        return alias.clone();
    }

    let base = format!("channel-{}", sanitize_repo_alias_component(channel_id));
    let mut alias = base.clone();
    let mut suffix = 2usize;
    while repo_aliases.contains_key(&alias) {
        alias = format!("{base}-{suffix}");
        suffix += 1;
    }
    repo_aliases.insert(alias.clone(), canonical_root);
    alias
}

fn sanitize_repo_alias_component(raw: &str) -> String {
    let sanitized = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "legacy".to_string()
    } else {
        sanitized
    }
}

fn is_safe_repo_alias(alias: &str) -> bool {
    !alias.is_empty()
        && alias
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

// ─────────────────────────────────────────────────────────────────────────────
// TELEGRAM API HELPERS (raw reqwest, no teloxide)
// ─────────────────────────────────────────────────────────────────────────────

/// Bot info returned by getMe.
struct BotInfo {
    first_name: String,
    username: String,
}

/// Update info from getUpdates.
struct UpdateInfo {
    chat_id: i64,
    from_name: String,
}

/// Validate a bot token via the Telegram getMe API.
async fn telegram_get_me(token: &str) -> Result<BotInfo> {
    let url = format!("https://api.telegram.org/bot{}/getMe", token);
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .context("Network error calling Telegram API")?;

    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse Telegram API response")?;

    if !status.is_success() || body.get("ok") != Some(&serde_json::Value::Bool(true)) {
        let description = body
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        anyhow::bail!("Telegram API error: {}", description);
    }

    let result = body
        .get("result")
        .context("Missing 'result' in Telegram response")?;
    let first_name = result
        .get("first_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let username = result
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown_bot")
        .to_string();

    Ok(BotInfo {
        first_name,
        username,
    })
}

/// Long-poll for the first message sent to the bot.
async fn telegram_get_updates(token: &str, timeout_secs: u64) -> Result<UpdateInfo> {
    let client = reqwest::Client::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    // Telegram long polling uses a max of 50 seconds per request
    let poll_timeout = std::cmp::min(timeout_secs, 30);
    let mut offset: Option<i64> = None;

    while std::time::Instant::now() < deadline {
        let remaining = deadline.duration_since(std::time::Instant::now()).as_secs();
        if remaining == 0 {
            break;
        }
        let this_timeout = std::cmp::min(poll_timeout, remaining);

        let mut url = format!(
            "https://api.telegram.org/bot{}/getUpdates?timeout={}",
            token, this_timeout
        );
        if let Some(off) = offset {
            url.push_str(&format!("&offset={}", off));
        }

        let resp = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(this_timeout + 10))
            .send()
            .await
            .context("Network error calling Telegram API")?;

        let body: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Telegram API response")?;

        if let Some(results) = body.get("result").and_then(|v| v.as_array()) {
            for update in results {
                // Track offset for next poll
                if let Some(update_id) = update.get("update_id").and_then(|v| v.as_i64()) {
                    offset = Some(update_id + 1);
                }

                // Extract message
                if let Some(message) = update.get("message") {
                    let chat_id = message
                        .get("chat")
                        .and_then(|c| c.get("id"))
                        .and_then(|v| v.as_i64());

                    let from_name = message
                        .get("from")
                        .and_then(|f| {
                            let first = f.get("first_name").and_then(|v| v.as_str());
                            let last = f.get("last_name").and_then(|v| v.as_str());
                            match (first, last) {
                                (Some(f), Some(l)) => Some(format!("{} {}", f, l)),
                                (Some(f), None) => Some(f.to_string()),
                                _ => None,
                            }
                        })
                        .unwrap_or_else(|| "Unknown".to_string());

                    if let Some(chat_id) = chat_id {
                        return Ok(UpdateInfo { chat_id, from_name });
                    }
                }
            }
        }
    }

    anyhow::bail!("Timed out waiting for a message ({}s)", timeout_secs)
}

/// Send a message to a Telegram chat.
pub(crate) async fn telegram_send_message(token: &str, chat_id: i64, text: &str) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
    });

    let resp = client
        .post(&url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .context("Network error calling Telegram API")?;

    let body: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse Telegram API response")?;

    if body.get("ok") != Some(&serde_json::Value::Bool(true)) {
        let description = body
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        anyhow::bail!("Telegram sendMessage failed: {}", description);
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// KEYCHAIN HELPERS
// ─────────────────────────────────────────────────────────────────────────────

/// Store bot token in OS keychain.
fn store_bot_token(token: &str) -> Result<()> {
    let entry = keyring::Entry::new("ralph", "telegram-bot-token")
        .context("Failed to create keychain entry")?;
    if let Err(err) = entry.set_password(token) {
        // Some keychains refuse overwrites; try delete + set as a fallback.
        if entry.delete_credential().is_ok() {
            entry
                .set_password(token)
                .context("Failed to store token in keychain after deleting existing entry")?;
        } else {
            return Err(anyhow::anyhow!(
                "Failed to store token in keychain: {}",
                err
            ));
        }
    }
    Ok(())
}

/// Load bot token from OS keychain.
fn load_bot_token() -> Option<String> {
    keyring::Entry::new("ralph", "telegram-bot-token")
        .ok()
        .and_then(|e| e.get_password().ok())
}

// ─────────────────────────────────────────────────────────────────────────────
// CONFIG HELPERS
// ─────────────────────────────────────────────────────────────────────────────

/// Save RObot config to ralph.yml.
///
/// If ralph.yml exists, parses it and updates the RObot section.
/// If it doesn't exist, creates a minimal config.
fn save_robot_config(timeout: u64, bot_token: Option<&str>) -> Result<()> {
    let config_path = Path::new("ralph.yml");

    let robot = serde_yaml::Value::Mapping({
        let mut m = serde_yaml::Mapping::new();
        m.insert(
            serde_yaml::Value::String("enabled".to_string()),
            serde_yaml::Value::Bool(true),
        );
        m.insert(
            serde_yaml::Value::String("timeout_seconds".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(timeout)),
        );
        if let Some(token) = bot_token {
            let mut telegram = serde_yaml::Mapping::new();
            telegram.insert(
                serde_yaml::Value::String("bot_token".to_string()),
                serde_yaml::Value::String(token.to_string()),
            );
            m.insert(
                serde_yaml::Value::String("telegram".to_string()),
                serde_yaml::Value::Mapping(telegram),
            );
        }
        m
    });

    if config_path.exists() {
        // Read existing config as raw YAML value to preserve structure
        let content = std::fs::read_to_string(config_path).context("Failed to read ralph.yml")?;

        let mut doc: serde_yaml::Value =
            serde_yaml::from_str(&content).context("Failed to parse ralph.yml")?;

        // Update or insert RObot section
        if let serde_yaml::Value::Mapping(ref mut map) = doc {
            map.insert(serde_yaml::Value::String("RObot".to_string()), robot);
        }

        let yaml_str = serde_yaml::to_string(&doc).context("Failed to serialize config")?;
        std::fs::write(config_path, yaml_str).context("Failed to write ralph.yml")?;
    } else {
        // Create minimal config
        let yaml = if let Some(token) = bot_token {
            format!(
                "RObot:\n  enabled: true\n  timeout_seconds: {}\n  telegram:\n    bot_token: {}\n",
                timeout, token
            )
        } else {
            format!("RObot:\n  enabled: true\n  timeout_seconds: {}\n", timeout)
        };
        std::fs::write(config_path, yaml).context("Failed to create ralph.yml")?;
    }

    Ok(())
}

fn save_slack_robot_config(
    timeout: u64,
    bot_token: Option<&str>,
    app_token: Option<&str>,
    channel_ids: &[String],
    allowed_users: &[String],
) -> Result<()> {
    let config_path = Path::new("ralph.yml");
    let doc = if config_path.exists() {
        let content = std::fs::read_to_string(config_path).context("Failed to read ralph.yml")?;
        serde_yaml::from_str(&content).context("Failed to parse ralph.yml")?
    } else {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
    };
    let mut root = match doc {
        serde_yaml::Value::Mapping(map) => map,
        _ => serde_yaml::Mapping::new(),
    };

    let mut slack_map = serde_yaml::Mapping::new();
    if let Some(token) = bot_token {
        slack_map.insert(
            serde_yaml::Value::String("bot_token".to_string()),
            serde_yaml::Value::String(token.to_string()),
        );
    }
    if let Some(token) = app_token {
        slack_map.insert(
            serde_yaml::Value::String("app_token".to_string()),
            serde_yaml::Value::String(token.to_string()),
        );
    }
    slack_map.insert(
        serde_yaml::Value::String("channel_ids".to_string()),
        serde_yaml::Value::Sequence(
            channel_ids
                .iter()
                .map(|id| serde_yaml::Value::String(id.clone()))
                .collect(),
        ),
    );
    slack_map.insert(
        serde_yaml::Value::String("allowed_users".to_string()),
        serde_yaml::Value::Sequence(
            allowed_users
                .iter()
                .map(|id| serde_yaml::Value::String(id.clone()))
                .collect(),
        ),
    );
    let cwd = std::env::current_dir()
        .context("Failed to resolve current directory for Slack channel_repos")?;
    let mut repo_aliases = serde_yaml::Mapping::new();
    repo_aliases.insert(
        serde_yaml::Value::String("ralph".to_string()),
        serde_yaml::Value::String(cwd.to_string_lossy().to_string()),
    );
    slack_map.insert(
        serde_yaml::Value::String("repo_aliases".to_string()),
        serde_yaml::Value::Mapping(repo_aliases),
    );
    let mut channel_repos = serde_yaml::Mapping::new();
    for channel_id in channel_ids {
        channel_repos.insert(
            serde_yaml::Value::String(channel_id.clone()),
            serde_yaml::Value::String("ralph".to_string()),
        );
    }
    slack_map.insert(
        serde_yaml::Value::String("channel_repos".to_string()),
        serde_yaml::Value::Mapping(channel_repos),
    );

    let mut robot_map = serde_yaml::Mapping::new();
    robot_map.insert(
        serde_yaml::Value::String("enabled".to_string()),
        serde_yaml::Value::Bool(true),
    );
    robot_map.insert(
        serde_yaml::Value::String("surface".to_string()),
        serde_yaml::Value::String("slack".to_string()),
    );
    robot_map.insert(
        serde_yaml::Value::String("timeout_seconds".to_string()),
        serde_yaml::Value::Number(serde_yaml::Number::from(timeout)),
    );
    robot_map.insert(
        serde_yaml::Value::String("slack".to_string()),
        serde_yaml::Value::Mapping(slack_map),
    );
    root.insert(
        serde_yaml::Value::String("RObot".to_string()),
        serde_yaml::Value::Mapping(robot_map),
    );

    let yaml_str = serde_yaml::to_string(&serde_yaml::Value::Mapping(root))
        .context("Failed to serialize config")?;
    std::fs::write(config_path, yaml_str).context("Failed to write ralph.yml")?;
    Ok(())
}

fn slack_config_path_from_sources(config_sources: &[ConfigSource]) -> PathBuf {
    config_sources
        .iter()
        .find_map(|source| match source {
            ConfigSource::File(path) => Some(path.clone()),
            ConfigSource::Builtin(_) | ConfigSource::Remote(_) | ConfigSource::Override { .. } => {
                None
            }
        })
        .unwrap_or_else(default_config_path)
}

fn load_slack_config_from(path: &Path) -> Option<ralph_core::SlackBotConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    let config: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    let slack = config
        .get("RObot")
        .or_else(|| config.get("robot"))
        .and_then(|r| r.get("slack"))?
        .clone();
    serde_yaml::from_value(slack).ok()
}

/// Write resolved config to a temporary runtime file so loop_runner receives a config path.
fn write_temp_config_for_daemon(workspace_root: &Path, config: &RalphConfig) -> Result<PathBuf> {
    let state_dir = workspace_root.join(".ralph");
    std::fs::create_dir_all(&state_dir).context("Failed to create .ralph directory")?;

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("Failed to generate runtime config filename: {e}"))?
        .as_nanos();
    let path = state_dir.join(format!(
        "daemon-config-{}-{}.yml",
        std::process::id(),
        nanos
    ));

    let yaml = serde_yaml::to_string(config).context("Failed to serialize runtime config")?;
    std::fs::write(&path, yaml).context("Failed to write temporary runtime config")?;

    Ok(path)
}

/// Save only the bot token into a config file, preserving other keys.
fn save_bot_token_config(path: &Path, token: &str) -> Result<()> {
    let doc = if path.exists() {
        let content = std::fs::read_to_string(path).context("Failed to read config file")?;
        serde_yaml::from_str(&content).context("Failed to parse config file")?
    } else {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
    };

    let mut root = match doc {
        serde_yaml::Value::Mapping(map) => map,
        _ => serde_yaml::Mapping::new(),
    };

    let robot_key = if root.contains_key("RObot") {
        serde_yaml::Value::String("RObot".to_string())
    } else if root.contains_key("robot") {
        serde_yaml::Value::String("robot".to_string())
    } else {
        serde_yaml::Value::String("RObot".to_string())
    };

    let mut robot_map = match root.get(&robot_key) {
        Some(serde_yaml::Value::Mapping(map)) => map.clone(),
        _ => serde_yaml::Mapping::new(),
    };

    let mut telegram_map = match robot_map.get("telegram") {
        Some(serde_yaml::Value::Mapping(map)) => map.clone(),
        _ => serde_yaml::Mapping::new(),
    };
    telegram_map.insert(
        serde_yaml::Value::String("bot_token".to_string()),
        serde_yaml::Value::String(token.to_string()),
    );
    robot_map.insert(
        serde_yaml::Value::String("telegram".to_string()),
        serde_yaml::Value::Mapping(telegram_map),
    );

    root.insert(robot_key, serde_yaml::Value::Mapping(robot_map));

    let yaml_str = serde_yaml::to_string(&serde_yaml::Value::Mapping(root))
        .context("Failed to serialize config")?;
    std::fs::write(path, yaml_str).context("Failed to write config file")?;
    Ok(())
}

/// Save telegram state with chat_id.
fn save_telegram_state(chat_id: i64) -> Result<()> {
    let state_dir = Path::new(".ralph");
    if !state_dir.exists() {
        std::fs::create_dir_all(state_dir).context("Failed to create .ralph directory")?;
    }

    let state = serde_json::json!({
        "chat_id": chat_id,
        "last_seen": null,
        "last_update_id": null,
        "pending_questions": {}
    });

    let state_path = state_dir.join("telegram-state.json");
    let content =
        serde_json::to_string_pretty(&state).context("Failed to serialize telegram state")?;
    std::fs::write(&state_path, format!("{}\n", content))
        .context("Failed to write telegram-state.json")?;

    Ok(())
}

/// Read bot token from a config file (legacy).
fn load_config_bot_token_from(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let config: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    config
        .get("RObot")
        .or_else(|| config.get("robot"))
        .and_then(|r| r.get("telegram"))
        .and_then(|t| t.get("bot_token"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Read bot token from ralph.yml (legacy).
fn load_config_bot_token() -> Option<String> {
    load_config_bot_token_from(Path::new("ralph.yml"))
}

/// Read custom Telegram API URL from a config file.
fn load_config_api_url_from(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let config: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    config
        .get("RObot")
        .or_else(|| config.get("robot"))
        .and_then(|r| r.get("telegram"))
        .and_then(|t| t.get("api_url"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Check if RObot is enabled in config.
fn is_robot_enabled() -> bool {
    let content = match std::fs::read_to_string("ralph.yml") {
        Ok(c) => c,
        Err(_) => return false,
    };
    let config: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(c) => c,
        Err(_) => return false,
    };
    config
        .get("RObot")
        .and_then(|r| r.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn normalize_token(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn resolve_token_from(
    env_token: Option<String>,
    keychain_token: Option<String>,
    config_token: Option<String>,
) -> Option<String> {
    normalize_token(env_token)
        .or_else(|| normalize_token(keychain_token))
        .or_else(|| normalize_token(config_token))
}

/// Resolve token from all sources (env > keychain > config).
pub(crate) fn resolve_token() -> Option<String> {
    resolve_token_from(
        std::env::var("RALPH_TELEGRAM_BOT_TOKEN").ok(),
        load_bot_token(),
        load_config_bot_token(),
    )
}

/// Resolve chat_id from telegram state.
pub(crate) fn resolve_chat_id() -> Option<i64> {
    let content = std::fs::read_to_string(".ralph/telegram-state.json").ok()?;
    let state: serde_json::Value = serde_json::from_str(&content).ok()?;
    state.get("chat_id").and_then(|v| v.as_i64())
}

// ─────────────────────────────────────────────────────────────────────────────
// INPUT HELPERS
// ─────────────────────────────────────────────────────────────────────────────

/// Prompt user for bot token with retry on empty input.
fn prompt_token() -> Result<String> {
    loop {
        print!("  Paste your bot token: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read input")?;
        let token = input.trim().to_string();
        if token.is_empty() {
            println!("  Token cannot be empty. Please try again.");
            continue;
        }
        return Ok(token);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OUTPUT HELPERS
// ─────────────────────────────────────────────────────────────────────────────

fn print_success(use_colors: bool, msg: &str) {
    if use_colors {
        println!("  \x1b[32m\u{2713}\x1b[0m {}", msg);
    } else {
        println!("  OK: {}", msg);
    }
}

fn print_error(use_colors: bool, msg: &str) {
    if use_colors {
        println!("  \x1b[31m\u{2717}\x1b[0m {}", msg);
    } else {
        println!("  ERROR: {}", msg);
    }
}

fn print_warning(use_colors: bool, msg: &str) {
    if use_colors {
        println!("  \x1b[33m!\x1b[0m {}", msg);
    } else {
        println!("  WARN: {}", msg);
    }
}

fn print_status(use_colors: bool, msg: &str) {
    if use_colors {
        println!("  \x1b[2m-\x1b[0m {}", msg);
    } else {
        println!("  {}", msg);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTS
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::CwdGuard;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn test_normalize_token_trims_and_discards_empty() {
        assert_eq!(normalize_token(None), None);
        assert_eq!(
            normalize_token(Some("  token-123  ".to_string())),
            Some("token-123".to_string())
        );
        assert_eq!(normalize_token(Some("   ".to_string())), None);
    }

    #[test]
    fn test_resolve_token_from_prefers_env_then_keychain_then_config() {
        let resolved = resolve_token_from(
            Some("  env-token  ".to_string()),
            Some("key-token".to_string()),
            Some("config-token".to_string()),
        );
        assert_eq!(resolved.as_deref(), Some("env-token"));

        let resolved = resolve_token_from(
            Some("   ".to_string()),
            Some("  key-token  ".to_string()),
            Some("config-token".to_string()),
        );
        assert_eq!(resolved.as_deref(), Some("key-token"));

        let resolved = resolve_token_from(None, None, Some("  cfg  ".to_string()));
        assert_eq!(resolved.as_deref(), Some("cfg"));
    }

    #[tokio::test]
    async fn test_run_daemon_rejects_builtin_config() {
        let sources = vec![ConfigSource::Builtin("tdd".to_string())];

        let err = run_daemon(DaemonArgs { slack: false }, &sources, None, false)
            .await
            .expect_err("expected daemon setup error");
        assert!(
            !err.to_string()
                .contains("Builtin presets are not supported"),
            "unexpected unsupported-config error: {err}"
        );
    }

    #[tokio::test]
    async fn test_run_daemon_rejects_remote_config() {
        let sources = vec![ConfigSource::Remote(
            "https://example.com/ralph.yml".to_string(),
        )];

        let err = run_daemon(DaemonArgs { slack: false }, &sources, None, false)
            .await
            .expect_err("expected remote config error");
        assert!(
            err.to_string()
                .contains("Failed to load config for bot daemon"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_run_daemon_errors_on_missing_config_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        let sources = vec![ConfigSource::File(PathBuf::from("missing.yml"))];
        let err = run_daemon(DaemonArgs { slack: false }, &sources, None, false)
            .await
            .expect_err("expected missing config error");
        assert!(
            err.to_string().contains("Config file not found"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_save_telegram_state_creates_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        save_telegram_state(123_456_789).expect("save telegram state");

        let state_path = temp_dir.path().join(".ralph").join("telegram-state.json");

        // Verify the file was created with correct content
        let read_content = std::fs::read_to_string(&state_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&read_content).unwrap();
        assert_eq!(
            parsed.get("chat_id").unwrap().as_i64().unwrap(),
            123_456_789
        );
        assert!(parsed.get("pending_questions").unwrap().is_object());
    }

    #[test]
    fn test_save_robot_config_creates_minimal_config_without_token() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        save_robot_config(180, None).expect("save robot config");

        let content = std::fs::read_to_string("ralph.yml").unwrap();
        let config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
        let robot = config.get("RObot").unwrap();
        assert!(robot.get("enabled").unwrap().as_bool().unwrap());
        assert_eq!(
            robot.get("timeout_seconds").and_then(|v| v.as_u64()),
            Some(180)
        );
        assert!(robot.get("telegram").is_none());
    }

    #[test]
    fn test_telegram_get_me_parses_response() {
        // Test JSON parsing logic (not actual API call)
        let body: serde_json::Value = serde_json::from_str(
            r#"{
                "ok": true,
                "result": {
                    "id": 123456,
                    "is_bot": true,
                    "first_name": "Ralph Bot",
                    "username": "ralph_test_bot"
                }
            }"#,
        )
        .unwrap();

        let result = body.get("result").unwrap();
        let first_name = result
            .get("first_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let username = result
            .get("username")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_bot");

        assert_eq!(first_name, "Ralph Bot");
        assert_eq!(username, "ralph_test_bot");
    }

    #[test]
    fn test_telegram_get_updates_parses_message() {
        // Test JSON parsing logic for update with message
        let body: serde_json::Value = serde_json::from_str(
            r#"{
                "ok": true,
                "result": [{
                    "update_id": 100,
                    "message": {
                        "message_id": 1,
                        "from": {
                            "id": 999,
                            "first_name": "John",
                            "last_name": "Doe"
                        },
                        "chat": {
                            "id": 999,
                            "type": "private"
                        },
                        "text": "hello"
                    }
                }]
            }"#,
        )
        .unwrap();

        let results = body.get("result").unwrap().as_array().unwrap();
        assert_eq!(results.len(), 1);

        let update = &results[0];
        let message = update.get("message").unwrap();
        let chat_id = message
            .get("chat")
            .unwrap()
            .get("id")
            .unwrap()
            .as_i64()
            .unwrap();
        assert_eq!(chat_id, 999);

        let from = message.get("from").unwrap();
        let first_name = from.get("first_name").unwrap().as_str().unwrap();
        let last_name = from.get("last_name").unwrap().as_str().unwrap();
        assert_eq!(format!("{} {}", first_name, last_name), "John Doe");
    }

    #[test]
    fn test_robot_config_yaml_generation() {
        // Test that we generate valid YAML for a minimal config
        let yaml = format!("RObot:\n  enabled: true\n  timeout_seconds: {}\n", 300);
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        let robot = parsed.get("RObot").unwrap();
        assert!(robot.get("enabled").unwrap().as_bool().unwrap());
        assert_eq!(robot.get("timeout_seconds").unwrap().as_u64().unwrap(), 300);
    }

    #[test]
    fn test_robot_config_update_preserves_existing() {
        // Test that updating an existing config preserves other fields
        let existing_yaml = "cli:\n  backend: claude\nevent_loop:\n  max_iterations: 50\n";
        let mut doc: serde_yaml::Value = serde_yaml::from_str(existing_yaml).unwrap();

        let robot = serde_yaml::Value::Mapping({
            let mut m = serde_yaml::Mapping::new();
            m.insert(
                serde_yaml::Value::String("enabled".to_string()),
                serde_yaml::Value::Bool(true),
            );
            m.insert(
                serde_yaml::Value::String("timeout_seconds".to_string()),
                serde_yaml::Value::Number(serde_yaml::Number::from(300_u64)),
            );
            m
        });

        if let serde_yaml::Value::Mapping(ref mut map) = doc {
            map.insert(serde_yaml::Value::String("RObot".to_string()), robot);
        }

        // Verify existing fields preserved
        assert!(doc.get("cli").is_some());
        assert!(doc.get("event_loop").is_some());
        // Verify RObot added
        let robot = doc.get("RObot").unwrap();
        assert!(robot.get("enabled").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_telegram_send_message_payload() {
        // Test that we build the correct JSON payload
        let payload = serde_json::json!({
            "chat_id": 123_456_789_i64,
            "text": "Hello from Ralph!",
        });

        assert_eq!(payload["chat_id"].as_i64().unwrap(), 123_456_789);
        assert_eq!(payload["text"].as_str().unwrap(), "Hello from Ralph!");
    }

    #[test]
    fn test_telegram_error_response_parsing() {
        let body: serde_json::Value = serde_json::from_str(
            r#"{
                "ok": false,
                "error_code": 401,
                "description": "Unauthorized"
            }"#,
        )
        .unwrap();

        let is_ok = body.get("ok") == Some(&serde_json::Value::Bool(true));
        assert!(!is_ok);

        let description = body
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        assert_eq!(description, "Unauthorized");
    }

    #[test]
    fn test_save_robot_config_with_token_writes_bot_token() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        save_robot_config(300, Some("test-token")).unwrap();

        let content = std::fs::read_to_string("ralph.yml").unwrap();
        let config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
        let token = config
            .get("RObot")
            .and_then(|r| r.get("telegram"))
            .and_then(|t| t.get("bot_token"))
            .and_then(|v| v.as_str());
        assert_eq!(token, Some("test-token"));
    }

    #[test]
    fn test_save_robot_config_updates_existing_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        std::fs::write("ralph.yml", "cli:\n  backend: claude\n").unwrap();

        save_robot_config(120, None).unwrap();

        let content = std::fs::read_to_string("ralph.yml").unwrap();
        let config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
        assert!(config.get("cli").is_some());
        let robot = config.get("RObot").unwrap();
        assert_eq!(
            robot.get("timeout_seconds").and_then(|v| v.as_u64()),
            Some(120)
        );
    }

    #[test]
    fn test_load_config_bot_token_from_reads_token() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("custom.yml");
        let yaml = "RObot:\n  telegram:\n    bot_token: token-123\n";
        std::fs::write(&config_path, yaml).unwrap();

        let token = load_config_bot_token_from(&config_path);
        assert_eq!(token.as_deref(), Some("token-123"));
    }

    #[test]
    fn test_load_config_bot_token_from_reads_lowercase_robot() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("custom.yml");
        let yaml = "robot:\n  telegram:\n    bot_token: token-lower\n";
        std::fs::write(&config_path, yaml).unwrap();

        let token = load_config_bot_token_from(&config_path);
        assert_eq!(token.as_deref(), Some("token-lower"));
    }

    #[test]
    fn test_save_bot_token_config_writes_token() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.yml");

        save_bot_token_config(&config_path, "new-token").unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
        let token = config
            .get("RObot")
            .and_then(|r| r.get("telegram"))
            .and_then(|t| t.get("bot_token"))
            .and_then(|v| v.as_str());
        assert_eq!(token, Some("new-token"));
    }

    #[test]
    fn test_save_bot_token_config_preserves_existing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.yml");
        let yaml = "cli:\n  backend: claude\nRObot:\n  enabled: true\n";
        std::fs::write(&config_path, yaml).unwrap();

        save_bot_token_config(&config_path, "new-token").unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
        assert!(config.get("cli").is_some());
        let robot = config.get("RObot").unwrap();
        assert_eq!(robot.get("enabled").and_then(|v| v.as_bool()), Some(true));
        let token = robot
            .get("telegram")
            .and_then(|t| t.get("bot_token"))
            .and_then(|v| v.as_str());
        assert_eq!(token, Some("new-token"));
    }

    #[test]
    fn test_save_bot_token_config_updates_lowercase_robot_key() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.yml");
        let yaml = "robot:\n  enabled: true\n";
        std::fs::write(&config_path, yaml).unwrap();

        save_bot_token_config(&config_path, "token-xyz").unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
        let token = config
            .get("robot")
            .and_then(|r| r.get("telegram"))
            .and_then(|t| t.get("bot_token"))
            .and_then(|v| v.as_str());
        assert_eq!(token, Some("token-xyz"));
        assert!(config.get("RObot").is_none());
    }

    #[test]
    fn test_load_config_bot_token_reads_legacy_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());
        std::fs::write(
            temp_dir.path().join("ralph.yml"),
            "RObot:\n  telegram:\n    bot_token: legacy-token\n",
        )
        .unwrap();

        assert_eq!(load_config_bot_token().as_deref(), Some("legacy-token"));
    }

    #[test]
    fn test_is_robot_enabled_reads_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());
        std::fs::write(
            temp_dir.path().join("ralph.yml"),
            "RObot:\n  enabled: true\n",
        )
        .unwrap();

        assert!(is_robot_enabled());

        std::fs::write(
            temp_dir.path().join("ralph.yml"),
            "RObot:\n  enabled: false\n",
        )
        .unwrap();

        assert!(!is_robot_enabled());
    }

    #[test]
    fn test_resolve_chat_id_reads_state_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());
        std::fs::create_dir_all(".ralph").unwrap();
        std::fs::write(
            ".ralph/telegram-state.json",
            r#"{"chat_id": 4242, "pending_questions": {}}"#,
        )
        .unwrap();

        assert_eq!(resolve_chat_id(), Some(4242));
    }

    #[test]
    fn test_resolve_chat_id_missing_file_returns_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        assert_eq!(resolve_chat_id(), None);
    }

    #[test]
    fn test_resolve_token_from_prefers_env_and_trims() {
        let resolved = resolve_token_from(
            Some("  env-token  ".to_string()),
            Some("keychain-token".to_string()),
            Some("config-token".to_string()),
        );

        assert_eq!(resolved.as_deref(), Some("env-token"));
    }

    #[test]
    fn test_resolve_token_from_skips_empty_values() {
        let resolved = resolve_token_from(
            Some("   ".to_string()),
            Some(String::new()),
            Some(" config-token ".to_string()),
        );

        assert_eq!(resolved.as_deref(), Some("config-token"));
    }

    #[test]
    fn test_resolve_token_from_returns_none_when_all_empty() {
        let resolved = resolve_token_from(
            Some("   ".to_string()),
            Some(String::new()),
            Some("   ".to_string()),
        );

        assert_eq!(resolved, None);
    }

    #[test]
    fn test_is_robot_enabled_missing_config_returns_false() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        assert!(!is_robot_enabled());
    }

    #[test]
    fn test_is_robot_enabled_invalid_yaml_returns_false() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());
        std::fs::write(temp_dir.path().join("ralph.yml"), "not: [valid").unwrap();

        assert!(!is_robot_enabled());
    }

    #[test]
    fn test_resolve_chat_id_invalid_json_returns_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());
        std::fs::create_dir_all(".ralph").unwrap();
        std::fs::write(".ralph/telegram-state.json", "not-json").unwrap();

        assert_eq!(resolve_chat_id(), None);
    }

    #[test]
    fn test_slack_cli_flags_parse() {
        let args = BotArgs::try_parse_from([
            "ralph",
            "onboard",
            "--slack",
            "--token",
            "xoxb-test",
            "--app-token",
            "xapp-test",
            "--channel",
            "C123",
            "--user",
            "U123",
        ])
        .unwrap();
        let BotCommands::Onboard(onboard) = args.command else {
            panic!("expected onboard command");
        };
        assert!(onboard.slack);
        assert_eq!(onboard.channel_ids, vec!["C123"]);
        assert_eq!(onboard.allowed_users, vec!["U123"]);
    }

    #[test]
    fn test_slack_archive_cli_flags_parse() {
        let args = BotArgs::try_parse_from([
            "ralph",
            "slack-archives",
            "prune",
            "--older-than-days",
            "14",
        ])
        .unwrap();
        let BotCommands::SlackArchives(archives) = args.command else {
            panic!("expected slack archives command");
        };
        let SlackArchiveCommands::Prune { older_than_days } = archives.command else {
            panic!("expected prune command");
        };
        assert_eq!(older_than_days, 14);
    }

    #[test]
    fn test_resolve_slack_repo_routing_uses_safe_aliases() {
        let repo = tempfile::tempdir().unwrap();
        let slack = ralph_core::SlackBotConfig {
            channel_ids: vec!["C123".to_string()],
            allowed_users: vec!["U123".to_string()],
            repo_aliases: HashMap::from([("ralph".to_string(), repo.path().to_path_buf())]),
            channel_repos: HashMap::from([("C123".to_string(), "ralph".to_string())]),
            ..Default::default()
        };

        let (repo_aliases, channel_repos) = resolve_slack_repo_routing(&slack).unwrap();

        assert_eq!(channel_repos.get("C123").map(String::as_str), Some("ralph"));
        assert_eq!(
            repo_aliases.get("ralph"),
            Some(&repo.path().canonicalize().unwrap())
        );
    }

    #[test]
    fn test_resolve_slack_repo_routing_accepts_legacy_absolute_channel_repos() {
        let repo = tempfile::tempdir().unwrap();
        let slack = ralph_core::SlackBotConfig {
            channel_ids: vec!["C123".to_string()],
            allowed_users: vec!["U123".to_string()],
            channel_repos: HashMap::from([(
                "C123".to_string(),
                repo.path().to_string_lossy().to_string(),
            )]),
            ..Default::default()
        };

        let (repo_aliases, channel_repos) = resolve_slack_repo_routing(&slack).unwrap();

        let alias = channel_repos.get("C123").unwrap();
        assert_eq!(alias, "channel-C123");
        assert_eq!(
            repo_aliases.get(alias),
            Some(&repo.path().canonicalize().unwrap())
        );
    }

    #[test]
    fn test_resolve_slack_repo_routing_rejects_unknown_non_absolute_selector() {
        let slack = ralph_core::SlackBotConfig {
            channel_ids: vec!["C123".to_string()],
            allowed_users: vec!["U123".to_string()],
            channel_repos: HashMap::from([("C123".to_string(), "missing".to_string())]),
            ..Default::default()
        };

        let err = resolve_slack_repo_routing(&slack).expect_err("expected unknown alias error");

        assert!(
            err.to_string()
                .contains("Slack channel C123 references unknown repo alias missing"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_save_slack_robot_config_writes_provider_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        save_slack_robot_config(
            600,
            Some("xoxb-test"),
            Some("xapp-test"),
            &["C123".to_string()],
            &["U123".to_string()],
        )
        .unwrap();

        let content = std::fs::read_to_string("ralph.yml").unwrap();
        let config: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
        let robot = config.get("RObot").unwrap();
        assert_eq!(robot.get("surface").and_then(|v| v.as_str()), Some("slack"));
        assert_eq!(
            robot
                .get("slack")
                .and_then(|s| s.get("bot_token"))
                .and_then(|v| v.as_str()),
            Some("xoxb-test")
        );
        let loaded = load_slack_config_from(Path::new("ralph.yml")).unwrap();
        assert_eq!(loaded.channel_ids, vec!["C123"]);
        assert_eq!(loaded.allowed_users, vec!["U123"]);
    }

    #[test]
    fn test_load_config_bot_token_from_missing_file_returns_none() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing_path = temp_dir.path().join("missing.yml");

        assert_eq!(load_config_bot_token_from(&missing_path), None);
    }
}
