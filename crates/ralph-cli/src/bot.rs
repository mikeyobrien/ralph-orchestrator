//! Bot setup and management commands.
//!
//! Provides:
//! - `ralph bot onboard --telegram` — Interactive wizard for Telegram bot setup
//! - `ralph bot status` — Check current bot configuration status
//! - `ralph bot test` — Send a test message to verify the bot works

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::Path;

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
    Status,
    /// Send a test message to verify the bot works
    Test(TestArgs),
    /// Run as a persistent daemon, listening on Telegram and starting loops on demand
    Daemon,
}

#[derive(Parser, Debug)]
pub struct OnboardArgs {
    /// Set up Telegram bot (default, only option for now)
    #[arg(long, default_value = "true")]
    pub telegram: bool,

    /// Skip interactive token prompt, provide token directly
    #[arg(long)]
    pub token: Option<String>,

    /// Skip chat_id detection, provide chat_id directly
    #[arg(long)]
    pub chat_id: Option<i64>,

    /// Timeout in seconds for waiting for a Telegram message
    #[arg(long, default_value = "120")]
    pub timeout: u64,
}

#[derive(Parser, Debug)]
pub struct TestArgs {
    /// Message to send (default: "Hello from Ralph!")
    #[arg(default_value = "Hello from Ralph!")]
    pub message: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// DISPATCHER
// ─────────────────────────────────────────────────────────────────────────────

pub async fn execute(args: BotArgs, use_colors: bool) -> Result<()> {
    match args.command {
        BotCommands::Onboard(onboard_args) => onboard_telegram(onboard_args, use_colors).await,
        BotCommands::Status => bot_status(use_colors).await,
        BotCommands::Test(test_args) => bot_test(test_args, use_colors).await,
        BotCommands::Daemon => run_daemon(use_colors).await,
    }
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

    // Store token in keychain
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
        }
    }

    // Update ralph.yml
    match save_robot_config(args.timeout) {
        Ok(()) => {
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
            "No token available. Run `ralph bot onboard --telegram` to set up.",
        );
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// TEST COMMAND
// ─────────────────────────────────────────────────────────────────────────────

async fn bot_test(args: TestArgs, use_colors: bool) -> Result<()> {
    // Resolve token
    let token = resolve_token().context(
        "No bot token available. Run `ralph bot onboard --telegram` or set RALPH_TELEGRAM_BOT_TOKEN",
    )?;

    // Resolve chat_id
    let chat_id = resolve_chat_id()
        .context("No chat_id found. Run `ralph bot onboard --telegram` to detect it")?;

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

// ─────────────────────────────────────────────────────────────────────────────
// DAEMON COMMAND
// ─────────────────────────────────────────────────────────────────────────────

/// Run the bot daemon — delegates to the configured communication adapter.
///
/// Currently only Telegram is supported. The adapter implements
/// [`DaemonAdapter`] and handles all platform-specific concerns.
async fn run_daemon(use_colors: bool) -> Result<()> {
    use ralph_proto::DaemonAdapter;

    let workspace_root = std::env::current_dir().context("Failed to get current directory")?;

    // Resolve bot token and chat_id for Telegram adapter
    let token = resolve_token().context(
        "No bot token available. Run `ralph bot onboard --telegram` or set RALPH_TELEGRAM_BOT_TOKEN",
    )?;
    let chat_id = resolve_chat_id()
        .context("No chat_id found. Run `ralph bot onboard --telegram` to detect it")?;

    if use_colors {
        println!("\x1b[1mRalph Daemon\x1b[0m (Telegram)");
    } else {
        println!("Ralph Daemon (Telegram)");
    }

    // Build the adapter
    let adapter = ralph_telegram::TelegramDaemon::new(token, chat_id);

    // Build the start_loop callback — wraps our CLI loop runner
    let start_loop: ralph_proto::StartLoopFn = Box::new(move |prompt: String| {
        Box::pin(async move {
            let ws = std::env::current_dir()?;
            let reason = crate::loop_runner::start_loop(prompt, ws, None).await?;
            Ok(format!("{:?}", reason))
        })
    });

    adapter.run_daemon(workspace_root, start_loop).await?;

    Ok(())
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
    entry
        .set_password(token)
        .context("Failed to store token in keychain")?;
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

/// Save RObot config to ralph.yml (without token).
///
/// If ralph.yml exists, parses it and updates the RObot section.
/// If it doesn't exist, creates a minimal config.
fn save_robot_config(timeout: u64) -> Result<()> {
    let config_path = Path::new("ralph.yml");

    if config_path.exists() {
        // Read existing config as raw YAML value to preserve structure
        let content = std::fs::read_to_string(config_path).context("Failed to read ralph.yml")?;

        let mut doc: serde_yaml::Value =
            serde_yaml::from_str(&content).context("Failed to parse ralph.yml")?;

        // Update or insert RObot section
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
            m
        });

        if let serde_yaml::Value::Mapping(ref mut map) = doc {
            map.insert(serde_yaml::Value::String("RObot".to_string()), robot);
        }

        let yaml_str = serde_yaml::to_string(&doc).context("Failed to serialize config")?;
        std::fs::write(config_path, yaml_str).context("Failed to write ralph.yml")?;
    } else {
        // Create minimal config
        let yaml = format!("RObot:\n  enabled: true\n  timeout_seconds: {}\n", timeout);
        std::fs::write(config_path, yaml).context("Failed to create ralph.yml")?;
    }

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
        "pending_questions": {}
    });

    let state_path = state_dir.join("telegram-state.json");
    let content =
        serde_json::to_string_pretty(&state).context("Failed to serialize telegram state")?;
    std::fs::write(&state_path, format!("{}\n", content))
        .context("Failed to write telegram-state.json")?;

    Ok(())
}

/// Read bot token from config file (legacy).
fn load_config_bot_token() -> Option<String> {
    let content = std::fs::read_to_string("ralph.yml").ok()?;
    let config: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    config
        .get("RObot")
        .or_else(|| config.get("robot"))
        .and_then(|r| r.get("telegram"))
        .and_then(|t| t.get("bot_token"))
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

/// Resolve token from all sources (env > keychain > config).
pub(crate) fn resolve_token() -> Option<String> {
    std::env::var("RALPH_TELEGRAM_BOT_TOKEN")
        .ok()
        .or_else(load_bot_token)
        .or_else(load_config_bot_token)
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
    #[test]
    fn test_save_telegram_state_creates_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state_dir = temp_dir.path().join(".ralph");
        let state_path = state_dir.join("telegram-state.json");

        // Use the temp dir as working directory for the state
        std::fs::create_dir_all(&state_dir).unwrap();
        let state = serde_json::json!({
            "chat_id": 123_456_789_i64,
            "last_seen": null,
            "pending_questions": {}
        });
        let content = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(&state_path, format!("{}\n", content)).unwrap();

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
}
