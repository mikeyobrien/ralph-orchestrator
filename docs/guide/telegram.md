# Telegram Integration

Ralph supports human-in-the-loop communication via Telegram. Agents can ask questions during orchestration, and humans can send proactive guidance at any time — all through a Telegram bot.

## Setup

### 1. Create a Telegram Bot

1. Open Telegram and message [@BotFather](https://t.me/BotFather)
2. Send `/newbot` and follow the prompts
3. Copy the bot token (format: `123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11`)

### 2. Configure Ralph

**Option A: Environment variable (recommended)**

```bash
export RALPH_TELEGRAM_BOT_TOKEN="your-bot-token"
```

**Option B: Config file**

```yaml
# ralph.yml
RObot:
  enabled: true
  timeout_seconds: 300
  telegram:
    bot_token: "your-bot-token"
```

The environment variable takes precedence over the config file.

### 3. Start a Loop

```bash
ralph run -p "your prompt"
```

The bot sends a greeting message on startup. The chat ID is auto-detected from the first message you send to the bot — just send any message to get started.

## Configuration Reference

```yaml
RObot:
  enabled: true                    # Enable human-in-the-loop (default: false)
  timeout_seconds: 300             # How long to block waiting for a response
  checkin_interval_seconds: 120    # Periodic status updates (optional)
  telegram:
    bot_token: "your-bot-token"    # Or use RALPH_TELEGRAM_BOT_TOKEN env var
```

| Field | Required | Description |
|-------|----------|-------------|
| `enabled` | Yes | Must be `true` to activate Telegram |
| `timeout_seconds` | Yes | Seconds to wait for a human reply before continuing |
| `checkin_interval_seconds` | No | Send periodic "still working" status updates |
| `telegram.bot_token` | Yes* | Bot token from BotFather (*or set via env var) |

For long-running loops, increase `timeout_seconds` and set `checkin_interval_seconds`:

```yaml
RObot:
  enabled: true
  timeout_seconds: 43200            # 12 hours
  checkin_interval_seconds: 900     # Check in every 15 minutes
```

## How It Works

### Agent Asks a Question (`human.interact`)

When an agent emits a `human.interact` event during orchestration:

1. The bot formats the question with context (hat name, iteration, loop ID) and sends it to Telegram
2. The event loop **blocks**, waiting for a reply
3. You reply in Telegram
4. Your reply is published as a `human.response` event
5. The next iteration receives your response in its context

If no reply arrives within `timeout_seconds`, the loop continues without a response.

### You Send Proactive Guidance (`human.guidance`)

You can send messages at any time (not as replies to a question):

1. Your message is written as a `human.guidance` event to `events.jsonl`
2. On the next iteration, all guidance events are collected and squashed into a numbered list
3. A `## ROBOT GUIDANCE` section is injected into the agent's prompt

This lets you steer the agent without waiting for it to ask.

### Event Summary

| Event | Direction | Behavior |
|-------|-----------|----------|
| `human.interact` | Agent to Human | Agent asks a question; loop blocks until reply or timeout |
| `human.response` | Human to Agent | Your reply to a `human.interact` question |
| `human.guidance` | Human to Agent | Proactive message injected into agent's next prompt |

## Parallel Loop Routing

When running multiple loops in parallel (via worktrees), messages are routed by priority:

1. **Reply-to**: Replying to a bot question routes to the loop that asked it
2. **@prefix**: Starting a message with `@loop-id` routes to that specific loop
3. **Default**: Messages without routing go to the primary loop

Examples:

- Reply directly to a question message → routed to the loop that asked
- Send `@feature-auth check the edge cases` → routed to the `feature-auth` loop
- Send `focus on tests` → routed to the primary (main) loop

Each loop has its own `events.jsonl`:
- Primary loop: `.ralph/events.jsonl`
- Worktree loops: `.worktrees/<loop-id>/.ralph/events.jsonl`

## Multimedia Support

The Telegram integration supports sending files and images:

- **Documents**: Any file type (logs, reports, etc.)
- **Photos**: Image files with optional HTML-formatted captions

Both support retry with exponential backoff, same as text messages.

## Bot Behavior

### Lifecycle

- **Startup**: Sends a greeting message if the chat ID is known
- **Running**: Polls for incoming messages via long polling (`getUpdates`)
- **Shutdown**: Sends a farewell message, then stops the polling task

### Reactions

The bot reacts to your messages with emoji:
- **Replies to questions**: reacted with a thumbs up
- **Proactive guidance**: reacted with eyes, plus a short text acknowledgment

### Primary Loop Only

The Telegram bot only starts on the **primary loop** (the one holding `.ralph/loop.lock`). Worktree loops route messages through the primary loop's bot.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Send failure | Retried with exponential backoff: 1s, 2s, 4s (3 attempts) |
| All retries fail | Logged to diagnostics, treated as timeout (loop continues) |
| Missing bot token | Clear error listing both config and env var options |
| Response timeout | Configurable via `timeout_seconds`; loop continues without response |
| No chat ID | Questions logged but not sent; resolved when you message the bot |

## State File

The bot persists its state to `.ralph/telegram-state.json`:

```json
{
  "chat_id": 123456789,
  "last_seen": "2026-01-29T10:00:00Z",
  "pending_questions": {
    "main": {
      "asked_at": "2026-01-29T10:05:00Z",
      "message_id": 42
    }
  }
}
```

- `chat_id`: Auto-detected from your first message to the bot
- `pending_questions`: Tracks which loops have outstanding questions, used for reply routing

## Architecture

```
TelegramService (lifecycle management)
├── BotApi / TelegramBot (teloxide wrapper, sends messages/documents/photos)
├── StateManager (chat ID, pending questions, reply routing)
├── MessageHandler (incoming messages → events.jsonl)
└── retry_with_backoff (exponential retry for all sends)
```

The crate lives at `crates/ralph-telegram/` with these modules:

| Module | Purpose |
|--------|---------|
| `lib.rs` | Public API exports |
| `bot.rs` | `BotApi` trait + `TelegramBot` implementation, message formatting |
| `service.rs` | `TelegramService` lifecycle, send/receive, polling |
| `handler.rs` | `MessageHandler` for routing incoming messages to events |
| `state.rs` | `StateManager` + `TelegramState` persistence |
| `error.rs` | `TelegramError` enum with typed error variants |

## Testing

```bash
cargo test -p ralph-telegram          # 33 unit tests (mocked, no network)
cargo test -p ralph-core human        # 11 integration tests in ralph-core
```

All tests use a `MockBot` implementation of `BotApi` — no Telegram API calls are made during testing.

## Troubleshooting

### Bot doesn't respond

- Verify your bot token: `curl https://api.telegram.org/bot<TOKEN>/getMe`
- Make sure you've sent at least one message to the bot (for chat ID auto-detection)
- Check that `RObot.enabled: true` is set in your config

### Messages go to the wrong loop

- Use reply-to for routing to the loop that asked the question
- Use `@loop-id` prefix to target a specific loop
- Unrouted messages default to the primary loop

### Timeout before you can respond

- Increase `timeout_seconds` in your config
- For long tasks, set `checkin_interval_seconds` so you know the loop is still active

### "No chat ID configured" warnings

- The bot auto-detects your chat ID from the first message you send
- Send any message to the bot to establish the connection
- The chat ID is persisted in `.ralph/telegram-state.json`
