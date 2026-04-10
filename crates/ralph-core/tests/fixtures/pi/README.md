# Pi Backend Smoke Test Fixtures

Recorded sessions from the Pi coding agent backend (`pi` CLI).

## Recording

```bash
cargo run --bin ralph -- run -c ralph.pi.yml \
  --record-session crates/ralph-core/tests/fixtures/pi/basic_pi_session.jsonl \
  -p "Say hello and emit LOOP_COMPLETE" \
  --max-iterations 2
```

## Notes

Pi runs in non-PTY JSON streaming mode (`--mode json --no-session`), so
recorded fixtures contain only `_meta.*` and `bus.publish` events — no
`ux.terminal.write` entries. The smoke runner still validates the session
structure, event parsing, and termination flow.
