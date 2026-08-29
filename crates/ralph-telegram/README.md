# ralph-telegram

This crate is Ralph's Telegram bot surface for human-in-the-loop
orchestration. When `RObot.enabled` is set, the Autoloop engine path relays
`ask.pending` questions through this crate and returns answers via
`autoloop control respond`.

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
    api_url: "http://localhost:8081"  # Optional: custom Bot API URL
```

The environment variable takes precedence over the config file value.

#### Custom API URL

To target a mock Telegram server (e.g., for CI or local HIL testing), set a custom API URL:

```bash
export RALPH_TELEGRAM_API_URL="http://localhost:8081"
```

Or use `RObot.telegram.api_url` in the config file. See the [Telegram guide](../../docs/guide/telegram.md#testing-with-a-mock-telegram-server) for a full walkthrough.

### 3. Start the Standalone Bot Daemon

```bash
ralph bot daemon
```

The daemon can send its greeting to a known Telegram chat and serve the
supported commands below. It does not attach the relay to `ralph run`; enabling
`RObot` for a run emits the inactivity warning shown above.

## Bot Commands

Available commands while the standalone daemon is running:

- `/status` — current loop status
- `/tasks` — open tasks
- `/memories` — recent memories
- `/tail` — last 20 retained events
- `/model` — current backend/model (runtime or config fallback)
- `/models` — configured model options found in `ralph*.yml`
- `/restart` — write `.ralph/restart-requested` when a loop is running
- `/stop` — write `.ralph/stop-requested` when a loop is running
- `/help` — list available commands

## Reserved Relay Design (Not Current v3 Behavior)

### Reserved `human.interact` Flow

The following is the reserved design pending autoloop#345; it does **not** run
when an agent emits `human.interact` today:

1. The bot sends the question to Telegram with context (hat name, iteration, loop ID)
2. The event loop blocks waiting for a reply
3. The human replies in Telegram
4. The reply is published as a `human.response` event on the bus
5. The next iteration receives the response in its context

The reserved timeout behavior would continue the loop without a response when
no reply arrives within `timeout_seconds`.

### Reserved `human.guidance` Flow

The retained design for proactive guidance is also inactive under autoloop:

1. A message is written as a `human.guidance` event to `events.jsonl`
2. On the next iteration, guidance events are collected and squashed
3. A `## ROBOT GUIDANCE` section is injected into the agent's prompt

### Reserved Parallel Loop Routing

The retained multi-loop routing design is:

1. **Reply-to**: Replying to a bot question routes to the loop that asked it
2. **@prefix**: Starting a message with `@loop-id` routes to that loop
3. **Default**: Messages without routing go to the primary loop

These routes do not connect Telegram messages to v3 autoloop runs today.

## Architecture

```
TelegramService (lifecycle management)
├── BotApi / TelegramBot (Teloxide wrapper, send messages)
├── StateManager (chat ID, pending questions, reply routing)
├── MessageHandler (incoming messages → events.jsonl)
└── retry_with_backoff (exponential retry for sends)
```

### Key Types

| Type | Purpose |
|------|---------|
| `TelegramService` | Lifecycle: start, stop, send questions, wait for responses |
| `BotApi` | Trait for send_message; `TelegramBot` is the real impl, `MockBot` for tests |
| `StateManager` | Persists state to `.ralph/telegram-state.json` |
| `MessageHandler` | Writes `human.response` / `human.guidance` events to JSONL |
| `TelegramError` | Typed errors: MissingBotToken, Startup, Send, Receive, ResponseTimeout, State |

## Error Handling

- **Send failures**: Retried with exponential backoff (3 attempts: 1s, 2s, 4s delays)
- **All retries exhausted**: Logged to diagnostics, treated as timeout (loop continues)
- **Missing bot token**: Clear error message listing both config and env var options
- **Response timeout**: Configurable via `timeout_seconds`; loop continues without response

## Testing

```bash
cargo test -p ralph-telegram
cargo test -p ralph-core human
```
