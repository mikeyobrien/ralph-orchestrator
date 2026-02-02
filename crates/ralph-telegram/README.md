# ralph-telegram

Telegram integration for human-in-the-loop orchestration in Ralph.

Enables bidirectional communication between AI agents and humans during orchestration loops:

- **AI to Human**: Agents emit `human.interact` events; the bot sends questions to Telegram
- **Human to AI**: Humans reply or send proactive `human.guidance` via Telegram messages

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

The environment variable takes precedence over the config file value.

### 3. Start a Loop

```bash
ralph run -p "your prompt"
```

When the bot starts, it sends a greeting message to your Telegram chat. The chat ID is auto-detected from the first message you send to the bot.

## How It Works

## Bot Commands

Available commands while a loop is running:

- `/status` — current loop status
- `/tasks` — open tasks
- `/memories` — recent memories
- `/tail` — last 20 events
- `/restart` — restart the loop
- `/stop` — stop the loop at the next iteration boundary
- `/help` — list available commands

### human.interact Flow

When an agent emits a `human.interact` event:

1. The bot sends the question to Telegram with context (hat name, iteration, loop ID)
2. The event loop **blocks** waiting for a reply
3. The human replies in Telegram
4. The reply is published as a `human.response` event on the bus
5. The next iteration receives the response in its context

If no response arrives within `timeout_seconds`, the loop continues without a response.

### human.guidance Flow

Humans can send messages at any time (not as replies to questions):

1. Message is written as a `human.guidance` event to `events.jsonl`
2. On the next iteration, guidance events are collected and squashed
3. A `## ROBOT GUIDANCE` section is injected into the agent's prompt

### Parallel Loop Routing

With multiple loops running, messages are routed by:

1. **Reply-to**: Replying to a bot question routes to the loop that asked it
2. **@prefix**: Starting a message with `@loop-id` routes to that loop
3. **Default**: Messages without routing go to the primary loop

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
cargo test -p ralph-telegram     # 33 unit tests
cargo test -p ralph-core human   # 11 integration tests in ralph-core
```
