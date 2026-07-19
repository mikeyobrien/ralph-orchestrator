# Telegram Integration

!!! warning "Not connected to the v3 engine"
    Telegram human-in-the-loop relay is **inactive for `ralph run` under the
    autoloop engine**. The engine relay contract is pending autoloop#345.
    Enabling `RObot` does not currently make an autoloop-backed run pause for
    questions, consume replies, or inject proactive guidance.

The `ralph-telegram` crate, `ralph bot` command group, configuration shape, and
persisted bot state remain in the repository for the future relay. You can set
up credentials, inspect status, run the standalone daemon, and send a test
message; do not treat those checks as evidence that in-loop HITL works.

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
token and chat ID. It tests bot connectivity only; it does not activate the
missing autoloop relay.

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

## What Is Deferred

The retained code models the intended `human.interact`, `human.response`, and
`human.guidance` relay. Under the old in-house loop, those events were routed
between a running loop and Telegram. Under v3, autoloop owns the live event
loop, so Ralph cannot correctly block engine execution or inject replies until
autoloop exposes the relay contract tracked by autoloop#345.

Until that lands:

- Agents in `ralph run` cannot ask a human through the retained Telegram relay.
- Telegram replies and proactive messages are not injected into an autoloop
  iteration.
- Worktree-loop routing and periodic in-run check-ins are not active v3
  behavior.
- A successful `ralph bot test` only proves Telegram credentials and network
  access.

## Testing the Retained Crate

```bash
cargo test -p ralph-telegram
```

The crate tests use mocked bot behavior and do not require Telegram network
access. A custom `RALPH_TELEGRAM_API_URL` remains useful when developing the
retained bot components, but it cannot provide end-to-end v3 HITL while
#345 is unresolved.

## Troubleshooting Setup

### `ralph bot status` reports no token

Set `RALPH_TELEGRAM_BOT_TOKEN` or run `ralph bot onboard`.

### `ralph bot test` has no chat ID

Send a message to the bot during onboarding or provide `--chat-id` explicitly.

### The bot works, but `ralph run` does not pause or receive guidance

That is the expected v3 limitation pending autoloop#345, not a credential
problem.
