# Telegram Integration

When `RObot.enabled` is set, an autoloop-backed `ralph run` (and the bot
daemon) relays Autoloop `ask.pending` questions through Telegram and returns
answers with `autoloop control respond`. Proactive messages become
`human.guidance` and are forwarded with `control guide`. Human events are
written to `.ralph/human-events.jsonl`, not Autoloop's structured `--events`
stream. The in-process TUI still displays pending asks only.

## Setup

### 1. Create a Telegram Bot

1. Open Telegram and message [@BotFather](https://t.me/BotFather).
2. Send `/newbot` and follow the prompts.
3. Copy the bot token.

### 2. Run the Onboarding Command

```bash
ralph bot onboard
```

The command can prompt for the token and detect a chat ID, or accept the
current non-interactive options:

```bash
ralph bot onboard --token "$RALPH_TELEGRAM_BOT_TOKEN" --chat-id 123456789
```

Alternatively set `RALPH_TELEGRAM_BOT_TOKEN` and keep the reserved
configuration shape in `ralph.yml`:

```yaml
RObot:
  enabled: true
  timeout_seconds: 300
  telegram:
    bot_token: "your-bot-token"
```

The environment variable takes precedence over the token in the config file.
A custom Telegram Bot API endpoint can be set with
`RALPH_TELEGRAM_API_URL` or `RObot.telegram.api_url`.

### 3. Inspect or Test the Bot Configuration

```bash
ralph bot status
ralph bot test "Hello from Ralph"
```

`ralph bot test` performs a Telegram network send and therefore needs a valid
token and chat ID. It tests bot connectivity. In-loop HITL also requires
`RObot.enabled` and an Autoloop build that implements `control respond`.

## Retained Components

The implementation remains under `crates/ralph-telegram/`:

| Module | Purpose |
|--------|---------|
| `bot.rs` | Telegram API abstraction and message formatting |
| `service.rs` | Bot service lifecycle and polling |
| `handler.rs` | Incoming-message handling |
| `state.rs` | Persisted chat and pending-question state |
| `error.rs` | Typed Telegram errors |

Bot state is stored in `.ralph/telegram-state.json`. The command group also
includes `ralph bot token` and `ralph bot daemon`; use `ralph bot --help` for
the current options.

## Testing

```bash
cargo test -p ralph-telegram
cargo test -p ralph-cli autoloop_robot
```

The crate tests use mocked bot behavior and do not require Telegram network
access. A custom `RALPH_TELEGRAM_API_URL` remains useful when developing against
a local mock server. End-to-end HITL additionally needs an Autoloop build that
implements `control respond`.

## Troubleshooting Setup

### `ralph bot status` reports no token

Set `RALPH_TELEGRAM_BOT_TOKEN` or run `ralph bot onboard`.

### `ralph bot test` has no chat ID

Send a message to the bot during onboarding or provide `--chat-id` explicitly.

### The bot works, but `ralph run` does not pause or receive guidance

Confirm `RObot.enabled: true` in `ralph.yml`, that the Autoloop binary
implements `control respond`, and that `.ralph/current-events` points at
`.ralph/human-events.jsonl` while the run is active.
