---
status: draft
created: 2026-11-12
related:
  - .ralph/specs/terminal-ui.spec.md
  - https://herdr.dev/docs/add-herdr-support/
  - https://herdr.dev/docs/socket-api/#agent-state-reporting
  - https://github.com/mikeyobrien/autoloop/blob/main/docs/features/operability.md#herdr-pane-status
---

# Herdr integration

## Goal

Make every `ralph run` started in a [Herdr](https://herdr.dev) pane show up as a Herdr agent: named after its loop, with live state (`working`, `blocked`, `done`) and progress in the sidebar, reachable with one command from any terminal.

## Context

Herdr is a terminal workspace manager for coding agents. It lists the agents running in its panes, rolls their state up to tabs and workspaces, and notifies when one finishes or needs a decision. An agent Herdr does not know natively joins that list by reporting itself through the `herdr` CLI. Ralph has no Herdr code today, so a Ralph run is a plain terminal to Herdr.

Herdr sets `HERDR_ENV=1`, `HERDR_PANE_ID`, `HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, `HERDR_SOCKET_PATH` and `HERDR_BIN_PATH` in every pane process. The reporting contract is in the two Herdr links above; `"$HERDR_BIN_PATH" <group> --help` is the authority for flags.

Prior art: [autoloop](https://github.com/mikeyobrien/autoloop) already reports to Herdr (`packages/cli/src/cli/herdr-sink.ts`, documented under "herdr pane status"). It reports whenever `HERDR_PANE_ID` is set, sends state with `--message`, sets the pane title, calls `herdr notification show` when the operator is needed and when the loop finishes, and releases the pane when a signal stops it. `.ralph/specs/autoloop-parity-inventory.md` lists a Herdr sink as low value for Ralph. Where this spec differs from that sink, the reason is in the tables below: the gate (R1, H13), the sidebar line (R6, H4, H19) and resume (Phase 3). The backend environment rule (R9) comes from V5.

## Herdr behaviour this design relies on

The H rows were checked on Herdr 0.9.5 in throwaway named sessions on Linux, with stand-in processes (`sleep`, `cat`, small scripts) holding the panes. The real-run check after the table repeats the ones the design leans on with real Ralph runs.

| # | Behaviour | Basis |
|---|---|---|
| H1 | `pane report-agent <pane> --source ralph --agent ralph --state <s> --seq <n>` makes the pane agent `ralph` and tracks `working`, `blocked`, `idle`. A report whose `--seq` is not higher than the last accepted one is ignored, including one that arrives after a release. | tested |
| H2 | `idle` reported after `working` on a pane outside the active tab reads as `done` until the pane is viewed; `agent focus` turns it into `idle`. On a pane in the active tab it read as `idle` at once. The sequence release, `working`, rename, `idle` also ends as `done`, with the name set; release followed directly by `idle` ends as `idle`. | tested |
| H3 | `agent rename <pane> <name>` gives an addressable name matching `[a-z][a-z0-9_-]{0,31}`. A name held by another live agent is refused with `agent_name_taken`. Release clears the name, and so does a report from another source on the same pane. | tested |
| H4 | With Herdr's default sidebar rows, the agent line shows the display name (`report-metadata --display-agent`) when one is reported, and the agent name otherwise. A state label shows only when the user adds `state_text` to `ui.sidebar.agents.rows`, and a token only when they add `$name`. The line has about 22 columns at the default sidebar width; longer text is cut with `…`. | tested, screen capture |
| H5 | A backend's Herdr integration that inherits `HERDR_*` takes the pane over. After the installed Claude hook (`~/.claude/hooks/herdr-agent-state.sh`, run by hand with a synthetic `SessionStart`) reported on a Ralph-held pane, Ralph's later state reports were ignored and its name was gone. With the `HERDR_*` variables removed the hook did nothing; with only `HERDR_PANE_ID` removed it also did nothing. After a server restart, Herdr started `claude` in a pane carrying such a session reference (seen once, with the reference sent over the socket). | tested; real run V5, V6 |
| H6 | A second self-reporter (`--source kiro --agent kiro`) on a Ralph-held pane replaces Ralph. Ralph's next report takes the pane back without its name. | tested |
| H7 | A process named `claude` joining the foreground of a Ralph-held pane did not change the pane's agent or stop Ralph's reports. | tested, stand-in binary |
| H8 | `agent prompt` and `agent send-keys` refuse a self-reported agent with `agent_not_ready`. `agent get`, `read`, `focus`, `wait`, `rename` and `attach` work. `pane send-text` delivers literal keystrokes to the pane's foreground process. | tested |
| H9 | A resume command is stored by `report-agent … -- <argv>` or `report-agent-session … -- <argv>`. Later state reports keep it, and no report removes it: an empty argv is rejected and a null one is ignored. `release-agent` removes it. It also went away, together with the agent, when Ralph exited to the shell while the server kept running. | tested; real run V9, V10 |
| H10 | After a server restart Herdr ran the stored command in the same pane in 4 of 5 restarts, in 3 of them before any client attached. The resumed process had `HERDR_*` set and re-claimed the pane by reporting. The miss was a restart during which sibling panes were closed; its cause is unknown. A real Ralph run came back 10 times out of 10. | tested; real run V8 |
| H11 | A resume argv containing an apostrophe or control character rejects the whole report. Its first word must be a plain command name, with at most 64 arguments and 8 KiB. | Herdr docs |
| H12 | Stopping a Herdr session sends `SIGHUP`, then `SIGTERM` about 0.4 seconds later, to the pane's foreground process and to a child with piped stdio in its own process group. Processes that caught both and kept running were gone within 4 seconds. | tested; real run V7 |
| H13 | A process started in a pane has the session id that `pane process-info --pane <id>` returns as `.result.process_info.shell_pid`. A process moved to a new session with `setsid` does not. | tested; real run V2 |
| H14 | From outside Herdr, `herdr --session <name> agent attach <agent>` showed the pane; with only `HERDR_SOCKET_PATH` naming a non-default session it exited 1. A refused CLI call exits 1 with a JSON error. | tested; real run V11 |
| H15 | `pane run <pane> "<line>"` types a shell command line; POSIX single-quote escaping survived zsh. `tab create --cwd … --label … --no-focus` returns `.result.root_pane.pane_id`. Without `--workspace` it used the server's existing workspace, and on a server with none it failed with `workspace_not_found`; `workspace create --cwd … --label … --no-focus` then returned a root pane. | tested |
| H16 | `--message` on `report-agent` is accepted but not returned by `pane get`, `agent get` or `api snapshot`. Popups are off unless the user sets `ui.toast.delivery`. | tested; Herdr config reference |
| H17 | Tokens set with `report-metadata` stay on the pane after `release-agent`. `--clear-token`, `--clear-title`, `--clear-display-agent` and `--clear-state-labels` remove what was set. | tested |
| H18 | With `ui.toast.delivery` unset, `working` to `idle` on a background tab drew no popup, and `herdr notification show` drew nothing on the client screen although it answered `shown`. Whether that call reaches the desktop some other way was not established. With `ui.toast.delivery = "herdr"`, `working` to `idle` on a background tab drew a popup `ralph finished`, and `working` to `blocked` drew `ralph needs attention`, each with a second line that locates the pane. Neither popup showed the `--message` text or the display name. `herdr notification show <title> --body <text>` drew a popup with both texts. | tested, screen capture |
| H19 | A title set with `report-metadata --title` is returned by `pane get` and drawn on the pane border when the tab holds more than one pane. It was not drawn for a single pane, in the sidebar with default rows, or while another tab was active. | tested, screen capture |

### Real-run check (2026-11-13)

Ralph 2.10.1, and 2.12.0 where a row says so, in throwaway Herdr 0.9.5 sessions, on Linux with zsh. Ralph has no Herdr code yet, so the calls of R4 to R6, R15 and R17 were issued by a script on four lifecycle hooks (`pre.loop.start`, `pre.iteration.start`, `post.loop.complete`, `post.loop.error`). The loop process runs those hooks, and it is the process R2 makes the reporter. The two R7 calls were run by the shell after Ralph had exited. R9 was stood in for by a wrapper that removes the six variables and then starts `claude`. Most runs used a script as the backend; three used the real Claude backend on a trivial task. The R1 gate, the R3 queue, R8 and all of Phase 2 do not exist yet and were not exercised.

| # | Result | Runs |
|---|---|---|
| V1 | Ralph as it is today is not an agent to Herdr: the pane has no agent, its status is `unknown`, and the sidebar lists nothing for it. | 1 |
| V2 | In a TUI run the TUI process sits on the pane's terminal with the pane's `shell_pid` as its session id. The `--rpc` loop child is in the same session, in its own process group, with pipes for stdin and stdout. The backend is in a session of its own on its own PTY. The same held for a run that Herdr had started from a resume command. This is what R1 and R2 assume. | 2, plus 1 resumed |
| V3 | The backend inherits the `HERDR_*` variables: all six in the script backend, and the three looked at (`HERDR_ENV`, `HERDR_PANE_ID`, `HERDR_SOCKET_PATH`) in the real `claude` processes. | 1 each |
| V4 | With the R4 to R6 calls coming from the loop process, the pane is agent `ralph`, `working`, and the sidebar line reads `ralph 1/6 ralph`, then `ralph 2/6 ralph`. The name `ralph-primary-<timestamp>` works with `agent get`, `agent focus` and `agent wait`. On a background tab a completed loop shows `done` and `ralph done 3 iters`, and focusing it turns it `idle`. A loop that hit max iterations shows `blocked` and `ralph max_iterations`, and `agent wait` returns `blocked`. After `q` and the two R7 calls the pane has no agent, display name, labels or tokens. One completed run on 2.12.0 showed the same `working` line and `ralph done 2 iters`, and exited 0 on `q`. | 1 completed, 1 on max iterations; 1 completed on 2.12.0 |
| V5 | Real Claude backend with the variables inherited, which is today's behaviour: about a second after Claude started, the pane carried a Claude session reference (source `herdr:claude`) and Ralph's agent name was gone. Ralph's `idle` report at completion exited 0 and the pane stayed `working`. The display line still updated. After a Herdr restart with Ralph's TUI still open, Herdr started an interactive `claude --resume <session>` in the pane, and the pane's agent was `claude`. | 1 |
| V6 | The same run with the backend started without the variables: no foreign session reference was seen (sampled once a second), the name stayed, the loop ended as `done`, and nothing was stored for Herdr to start after a restart. | 1 |
| V7 | Stopping the Herdr session mid-iteration left no process from the run behind. With real Claude in the middle of a tool call, none of the run's 32 processes (Ralph's two, Claude, and the helper and MCP processes Claude had started, in three sessions) was alive one second later, and a restart brought nothing back. No termination hook ran in the two stops where that was looked at, and the one history file looked at had no loop-end row. | script backend 3, `--no-tui` 1, real Claude 1 |
| V8 | Resume (R15) with the script backend, which writes the scratchpad itself. The command was stored at the second iteration start. After each of 10 stop and start cycles in a row, Herdr ran it in the same pane about 2 seconds after the server started. Each resumed run kept the loop id, reported under the same agent name, stored the command again and went on iterating. The flag order `run --continue --exclusive --loop-id <id> -c <file> -P <file>` parsed. | 1 loop, 10 cycles |
| V9 | A finished loop whose TUI is still open. With the R17 sequence the stored command was gone and a restart brought nothing back. Without it the command stayed, and the restart ran the finished loop again for one more iteration. | 1 each |
| V10 | Quitting a resumable loop with `q` mid-iteration: a few seconds later the agent and the stored command were both gone, with no release call made. A restart brought nothing back. | 2; restart checked in 1 |
| V11 | From a terminal outside Herdr, `herdr --session <name> agent attach <agent-name>` showed the live TUI. Closing that terminal left the loop running. A wrong session name gave `server_not_running` and exit 1. | 1 |

Three things about Ralph itself came up. None involves Herdr, and each touches a rule below.

| # | Finding | Basis |
|---|---|---|
| I1 | **An interrupt skips the loop's termination code.** `q` or Ctrl+C in the TUI mid-iteration, and `SIGINT` or `SIGTERM` sent to an `--autonomous` run, ended the loop with no `post.loop.*` hook and no loop-end row in the history. The TUI case also left a stale `.ralph/loop.lock`, which did not block the next run. The TUI command exited 1. The headless process exited on `SIGKILL` (status 137), and its backend, which is in another process group, was left running. Both interrupt branches in `loop_runner.rs` signal the process's own group with `SIGTERM` and then `SIGKILL` before the termination code. | TUI: 3 runs on 2.10.1 and 3 on 2.12.0. Headless: 4 runs over two 2.10.1 builds, and 3 on 2.12.0 (`SIGINT` twice, `SIGTERM` once). The cause is from reading, not instrumented. |
| I2 | **`q` during landing hangs.** When `q` is pressed after the loop has completed and while the loop child is still landing (the auto-commit), the TUI closes and both Ralph processes stay. The child sits in a read on its stdin pipe and the parent does not exit. The shell prompt does not return until Ctrl+C. `q` pressed after landing has finished exits normally. | 4 of 4 runs in which `q` arrived during landing. On 2.10.1: once by chance and once with a 12-second git pre-commit hook, lasting 500 and 164 seconds until Ctrl+C. On 2.12.0: twice with the same hook, both processes still there 45 seconds later, one of the two in a plain tmux pane with no `HERDR_*` variables. Cause not established. A likely one, from reading: `RpcWriter::close` calls `shutdown()`, which leaves the pipe open, and the child's stdin read cannot be cancelled. |
| I3 | **Short real runs wrote no scratchpad.** Neither completed Claude run wrote `.ralph/agent/scratchpad.md`, and `ralph run --continue` refuses to start without it. | 2 runs of one iteration each. Longer runs not tried. |

Not checked: a Ralph build that has this feature (there is none yet), backends other than Claude, a real backend across a resume (I3), a real backend on 2.12.0, Herdr's default session, `herdr update --handoff`, shells other than zsh, macOS and Windows.

## Non-goals (v1)

- **Panes for headless launchers.** The web runner (`backend/ralph-web-server/src/runner/ProcessSupervisor.ts`), `ralph-api` collection runs, the Telegram daemon (`start_loop`) and auto-merge (`process_pending_merges_with_command`) keep supervising by pid and log file.
- **Steering through Herdr.** `herdr agent prompt` refuses self-reported agents (H8). Raw `pane send-text` reaches the TUI as keystrokes. There `q` quits and the loop is aborted (seen in the real run, I1); that behaviour is not changed here.
- **Answering `human.interact` inside the pane.** The pane shows that the loop is blocked; the answer still goes through RObot.
- **A task-count token.** Nothing in the loop emits task counts today: `RpcEvent::TaskCountsUpdated` is defined and consumed but never sent.
- **Reporting from `hooks.events` scripts.** Lifecycle hooks can call `herdr` today, but they cannot stop a backend from taking the pane (H5).
- **Backend permission prompts.** When a backend waits on its own approval dialog inside the PTY, Ralph does not know and keeps reporting `working`.
- **A lasting `done` for `--no-tui` runs.** Such a run leaves the agent list when its process exits. A TUI run stays listed until the TUI is closed.

---

## Phase 1: Agent pane

### When Ralph reports

- **R1. Gate.** The decision is made once, at the CLI entry for `ralph run`. In-process callers such as `start_loop` never report. A process reports only when `HERDR_BIN_PATH` is set, `RALPH_HERDR` is not `0`, `off` or `false`, and one of these holds:
  - the hidden flag `--herdr-pane <pane-id>` was passed (hidden like `--legacy-tui`), or
  - `HERDR_ENV=1`, `HERDR_PANE_ID` is set, stdin and stdout are both terminals, and the process's session id equals the pane's `shell_pid` (H13).

  The terminal test keeps a Ralph spawned by `cargo test` with piped stdio off the developer's own pane. The session test keeps a Ralph started under tmux, `script` or an agent harness's own PTY off the outer pane whose variables it inherited. The `process-info` lookup is the one Herdr call made before the loop starts, and it is bounded to 2 seconds. If it fails or returns no `shell_pid`, Ralph does not report. On platforms without session ids the session test is skipped.
- **R2. One reporter per loop.** The process that runs the event loop sends the loop's state and metadata. In the default subprocess TUI mode that is the `ralph run --rpc` child, whose stdio is piped, so the TUI parent (`run_subprocess_tui`) passes it `--herdr-pane "$HERDR_PANE_ID"` when the parent's own gate holds. A flag is used because a backend cannot inherit it the way it would an environment variable. The TUI parent itself sends only the reports in R7 and R8.
- **R3. Out of the way.** Once the loop has started, Herdr calls never delay it. Calls for one pane are issued in order, a newer report replaces a queued older one of the same kind, each call is bounded to 2 seconds, and failures are logged at debug level and otherwise ignored. At exit a reporter gives its queued calls at most 2 seconds in total, so that the terminal report is sent. A missing, hanging or failing `herdr` binary changes neither the run's result nor its exit code.

### What Ralph reports

- **R4. Identity.** At loop start: `report-agent --source ralph --agent ralph --state working`, then `agent rename <pane> <name>`. The name is `ralph-<loop-id>`, lowercased, with characters outside `[a-z0-9_-]` replaced by `-`, cut to 32 characters, and without a doubled prefix when the loop id already starts with `ralph-`. If the rename is refused the run continues unnamed. Every lifecycle report carries `--seq` set to the current time in nanoseconds.
- **R5. Summary.** One short string describes the loop's current situation:

  | Situation | Herdr state | Summary |
  |---|---|---|
  | Iteration running | `working` | `<iteration>/<max> <hat-id>` |
  | Waiting on `human.interact` with a robot service attached | `blocked` | `? <question, first line>` |
  | Suspended by a lifecycle hook | `blocked` | `suspended: <hook-name>` |
  | Ended: `CompletionPromise` | `idle` | `done <n> iters` |
  | Ended: `Cancelled`, `Interrupted`, `Stopped` | `idle` | `cancelled`, `interrupted`, `stopped` |
  | Ended: any other `TerminationReason` except `RestartRequested` | `blocked` | `TerminationReason::as_str()` |
  | Loop process exited without a terminal report | `blocked` | `loop exited` |

  An end that needs a human decision is `blocked` so that Herdr flags the pane and `herdr agent wait` returns. `RestartRequested` reports nothing. The mapping from `TerminationReason` is an exhaustive `match`, so a new variant fails to compile until it is placed.

  `Interrupted` is in the table for completeness. Today the loop process does not reach its termination code on an interrupt (I1), so nothing is sent for it. With a TUI, the R7 calls from the TUI parent follow at once. Without one, Herdr drops the agent when the process exits, and the three tokens stay on the pane until the next run overwrites them (seen when a TUI run exited with no release sent).

  The call sites in `crates/ralph-cli/src/loop_runner.rs`, found by reading: where `RpcEvent::IterationStart` is emitted; around the `process_events_from_jsonl_with_waves` call when `pending_human_interact_context_in_jsonl` returned a context; `wait_for_resume_if_suspended`; the `handle_termination` closure.
- **R6. Display.** On every summary change, after the state report if the state changed too, one `report-metadata --source ralph-display --agent ralph --applies-to-source ralph` call sets:
  - `--display-agent "ralph <summary>"`, the only text visible with Herdr's default sidebar (H4);
  - `--state-label <state>=<summary>` for the current state;
  - `--token loop=<loop-id>`, `--token hat=<hat-id>`, `--token iter=<iteration>/<max>`.

  A `blocked` report also passes `--message <summary>`.
- **R7. Leaving.** A process that reports and is not an `--rpc` child ends with two calls, once, before it exits: a `report-metadata --source ralph-display` that clears the display name, the state labels and the three tokens (H17), then `release-agent`. In subprocess TUI mode the TUI parent sends them after the child has exited and the TUI has closed, so a finished loop stays visible as `done` while its TUI is open. `run_command` ends failed runs with `std::process::exit`, so both calls have to happen before that. The real run found a case in which the child never exits (I2). As worded, the two calls are not sent in that case; see the open points.
- **R8. Loop process gone.** In subprocess TUI mode, when the child's event stream ends without a `LoopTerminated` event (the EOF branch of `run_rpc_event_reader`), the TUI parent reports `blocked` with summary `loop exited`. This applies while the TUI is open. Quitting the TUI aborts the loop child, whose stream then ends the same way (I1); nothing is reported for that, and R7 follows.

### Keeping the pane

- **R9. Backend environment.** Every process Ralph starts to run a backend for a loop has `HERDR_ENV`, `HERDR_PANE_ID`, `HERDR_WORKSPACE_ID`, `HERDR_TAB_ID`, `HERDR_SOCKET_PATH` and `HERDR_BIN_PATH` removed, whether or not Ralph itself is reporting. The spawn paths today:
  - `inject_ralph_runtime_env` in `crates/ralph-adapters/src/cli_executor.rs` and in `pty_executor.rs`;
  - `run_acp_lifecycle_inner` and `create_terminal` in `acp_executor.rs`;
  - `run_wave_worker_pty` in `crates/ralph-cli/src/loop_runner.rs`.

  `rg 'Command::new|CommandBuilder::new' crates/ralph-adapters crates/ralph-cli/src/loop_runner.rs` finds any added since. This is what stops H5 and H6.
- **R10. Left alone.** `sop_runner.rs` (`ralph plan`, `ralph code-task`) keeps the variables, because there the backend is the interactive agent and Herdr should see it. Lifecycle hook commands (`HookExecutor`) keep them too, so a user's hook can still call `herdr`.

### Acceptance criteria

The fake Herdr used below is an executable at `$HERDR_BIN_PATH` that appends its argv to a file, one JSON line per call.

#### AC1.1 Inactive outside Herdr
- **Given** no `HERDR_*` variables
- **When** a mock-backend run completes
- **Then** the fake Herdr was never invoked, and the run's output and exit code match a run without this feature

#### AC1.2 Piped stdio does not report
- **Given** `HERDR_ENV=1`, `HERDR_PANE_ID` and `HERDR_BIN_PATH` set, and piped stdio
- **When** a run completes
- **Then** the fake Herdr was never invoked

#### AC1.3 Lifecycle calls
- **Given** `--herdr-pane w9:p9` and a two-iteration mock-backend run without `--rpc`
- **When** the run completes
- **Then** the recorded calls include, in this order: `pane report-agent w9:p9 … --state working`; `agent rename w9:p9 ralph-<loop-id>`; a `pane report-metadata` with `--token iter=2/<max>`; `pane report-agent … --state idle`; a `pane report-metadata` with `--display-agent "ralph done 2 iters"`; a `pane report-metadata` with `--clear-token loop`; and `pane release-agent w9:p9` as the last call. `--seq` rises across the `report-agent` and `release-agent` calls

#### AC1.4 The loop process leaves the pane to its parent
- **Given** the AC1.3 run with `--rpc` added
- **When** the run completes
- **Then** no `release-agent` call and no `--clear-token` call is recorded

#### AC1.5 A limit is flagged
- **Given** a run that stops on max iterations
- **When** the run ends
- **Then** the last `report-agent` call has `--state blocked`, and a `report-metadata` call after it has `--display-agent "ralph max_iterations"`

#### AC1.6 Waiting on a human
- **Given** a pending `human.interact` and an attached robot service
- **When** the loop waits for the answer
- **Then** a `blocked` report and a display name starting `ralph ? ` are recorded before the wait, and a `working` report after it

#### AC1.7 Slow Herdr
- **Given** a fake Herdr that sleeps 30 seconds per call
- **When** a run completes
- **Then** it finishes within 5 seconds of the same run without Herdr, with the same exit code

#### AC1.8 Broken Herdr
- **Given** `HERDR_BIN_PATH` naming a missing file, or a fake Herdr that exits 1
- **When** a run completes
- **Then** the run succeeds and nothing about Herdr reaches stdout or stderr

#### AC1.9 Backend environment
- **Given** all six `HERDR_*` variables set in Ralph's environment
- **When** a backend script records its own environment
- **Then** none of the six is present, for each spawn path in R9

#### AC1.10 Off switch
- **Given** `RALPH_HERDR=0` and `--herdr-pane w9:p9`
- **When** a run completes
- **Then** the fake Herdr was never invoked

#### AC1.11 Agent names
- **Given** loop ids `primary-20261112-214500`, `bright-maple`, `ralph-20261112-214500-a3f2` and `My Loop!`
- **Then** the names are `ralph-primary-20261112-214500`, `ralph-bright-maple`, `ralph-20261112-214500-a3f2` and `ralph-my-loop-`, each matching `[a-z][a-z0-9_-]{0,31}`

#### AC1.12 Gate
- **Given** the gate decision as a pure function of its inputs
- **Then** it returns "report" only when the flag is given, or when `HERDR_ENV=1`, the pane id is set, both streams are terminals and the own session id equals the reported `shell_pid`. It returns "do not report" when the session ids differ, when `shell_pid` is missing, and when the lookup failed

---

## Phase 2: Launch and follow

- **R11. Follow.** `ralph loops follow [<loop-id>] [--session <name>]` resolves the loop (`resolve_loop`; with no id, the primary loop from `.ralph/current-loop-id`) and derives its agent name with the R4 function. Inside Herdr it runs `herdr agent focus <name>`. Outside it replaces itself with `herdr [--session <name>] agent attach <name>` (H14). When Herdr exits non-zero, the error names `ralph loops logs -f <loop-id>` as the fallback. The Herdr binary is `$HERDR_BIN_PATH` when set, otherwise `herdr` on `PATH`.
- **R12. Launch.** `ralph run --herdr <args>` starts the run in a new Herdr tab and returns without running a loop itself:
  1. `herdr tab create --cwd <cwd> --label <label> --no-focus`, with `--workspace "$HERDR_WORKSPACE_ID"` when that is set. The label is the run's existing prompt summary cut to 24 characters. If Herdr answers `workspace_not_found`, use `herdr workspace create --cwd <cwd> --label <label> --no-focus` and its root pane instead (H15).
  2. `herdr pane run <new-pane> <command line>`, where the command line is the current executable and `<args>` without `--herdr`, each POSIX single-quoted.
  3. Print the new pane id and the two follow commands, `herdr agent focus <pane>` and `herdr agent attach <pane>`.

  `--herdr=split` uses `herdr pane split "$HERDR_PANE_ID" --direction right --cwd <cwd> --no-focus` instead of a tab, and is an error outside Herdr. If any argument contains a newline or control character the command refuses and tells the user to pass the prompt with `-P <file>`. Any other Herdr call that exits non-zero ends the command non-zero with Herdr's message. Ralph never starts a Herdr server.
- **R13. Doctor.** `ralph doctor` prints one line: `herdr: active on pane <id>`, or `herdr: inactive (<which part of R1 failed>)`.
- **R14. Docs.** `docs/guide/herdr.md`, added to both guide lists in `mkdocs.yml`, covers: what appears in the sidebar and when; the R5 table; follow and launch; the two optional Herdr settings below; `RALPH_HERDR=0`; the non-goals a user would trip over.

  ```toml
  [ui.toast]
  delivery = "herdr"   # popups when a loop finishes or blocks; off by default

  [ui.sidebar.agents]
  rows = [["state_icon", "workspace", "tab"], ["agent"], ["$loop"]]
  ```

### Acceptance criteria

#### AC2.1 Follow inside Herdr
- **Given** `HERDR_ENV=1` and loop `bright-maple`
- **When** `ralph loops follow bright-maple` runs
- **Then** the fake Herdr records `agent focus ralph-bright-maple`

#### AC2.2 Follow from outside
- **Given** no `HERDR_ENV`
- **When** `ralph loops follow bright-maple --session work` runs
- **Then** the fake Herdr records `--session work agent attach ralph-bright-maple`

#### AC2.3 Follow fallback
- **Given** a fake Herdr that exits 1
- **When** `ralph loops follow bright-maple` runs
- **Then** it exits non-zero and its message contains `ralph loops logs -f`

#### AC2.4 Launch in a new tab
- **Given** `HERDR_WORKSPACE_ID=w1` and a fake Herdr answering `tab create` with a pane id
- **When** `ralph run --herdr -p "it's a test" --max-iterations 3` runs
- **Then** it records `tab create --workspace w1 … --no-focus`, then a `pane run` whose command line parses under `sh` to the original arguments without `--herdr`, and no loop runs in the calling process

#### AC2.5 Launch with no workspace
- **Given** a fake Herdr answering `tab create` with `workspace_not_found`
- **When** `ralph run --herdr -p x` runs
- **Then** `workspace create` is recorded next, and `pane run` targets its root pane

#### AC2.6 Launch refuses a multi-line prompt
- **Given** a prompt containing a newline
- **When** `ralph run --herdr -p …` runs
- **Then** it exits non-zero, names `-P`, and the fake Herdr was never invoked

#### AC2.7 Split needs a pane
- **Given** no `HERDR_ENV`
- **When** `ralph run --herdr=split -p x` runs
- **Then** it exits non-zero without invoking Herdr

---

## Phase 3: Resume after a Herdr restart

**Entry checks.** Start this phase only after Phase 1 has merged and the checks below pass in a throwaway named session with a real Ralph run. Each covers something that was not tested.

1. Stop the Herdr session while a loop is mid-iteration and start it again. The loop comes back in the same pane with `--continue` and reports as the same agent. `--continue` needs an existing scratchpad, so stop after the first iteration has written one.
2. After the stop and before the restart, no process from the old run is left: neither Ralph nor its backend.
3. Repeat ten times and count how often the command is re-run. H10 saw one unexplained miss in five.

State after the real-run check of 2026-11-13: check 1 passed with a script backend that writes the scratchpad itself, and is still open for a real backend because of I3 (V8). Check 2 passed, including once with real Claude (V7). Check 3 came out at 10 of 10 (V8).

- **R15. Resume command.** Once the loop id is known, a primary loop sends `report-agent-session --source ralph --agent ralph --agent-session-id <loop-id> -- <exe> run --continue --exclusive --loop-id <loop-id>`, followed by the original `-c`, `-H` and `-b` values and a `-P <file>` naming the prompt.
  - It is a call of its own, because an argv Herdr rejects discards the whole report it rides on (H11).
  - `<exe>` is the file name of the running executable. The command is sent only when that name resolves on `PATH` to the running executable.
  - It is sent at the first iteration boundary at which the scratchpad file exists, because `--continue` refuses to start without one.
  - An inline `-p` prompt is first written to a file under the loop's `.ralph` directory.
  - `--exclusive` makes a resumed run wait for the loop lock, where it would otherwise start a parallel loop in a worktree.
  - If any argument contains an apostrophe or control character, no resume command is sent.
- **R16. Primary loops only.** A worktree loop sends no resume command. `ralph run --continue` started at the repository root does not re-enter an existing worktree.
- **R17. Ended loops do not resume.** At any termination the reporter sends `release-agent`, then `report-agent --state working`, then the rename, then the terminal state and display from R5 and R6. While the TUI of an ended loop stays open its process is alive, and the resume command stays stored unless it is released (H9). In the real run a finished loop left that way was started again by a Herdr restart (V9). The `working` report in between is what lets the terminal `idle` read as `done` (H2).
- **R18. Hangup.** After `SIGHUP` the process makes no Herdr calls at all. A Herdr stop reaches the loop as `SIGHUP` (H12), and a release at that moment would delete the resume command that the restart needs. In the real run a Herdr stop ended the loop process before any of its termination code ran (V7, I1), so today the rule has nothing to hold back. It protects the resume command once I1 is fixed.

### Acceptance criteria

#### AC3.1 Resume command
- **Given** `--herdr-pane` and a primary loop started with `-c a.yml -b claude -p "do x"`
- **When** the first iteration has written the scratchpad
- **Then** the fake Herdr records a `pane report-agent-session` whose argv after `--` is `<exe> run --continue --exclusive --loop-id <id> -c a.yml -b claude -P <file>`, and `<file>` contains `do x`

#### AC3.2 Unsafe argument
- **Given** a prompt path containing an apostrophe
- **When** the loop runs
- **Then** no `report-agent-session` is recorded and the state reports are unaffected

#### AC3.3 Worktree loop
- **Given** a worktree loop
- **When** the loop runs
- **Then** no `report-agent-session` is recorded

#### AC3.4 Ended loop
- **Given** a loop that completes
- **When** it terminates
- **Then** the calls recorded at termination are `release-agent`, `report-agent --state working`, `agent rename`, `report-agent --state idle`, a `report-metadata`, and last the two R7 calls if the process owns the pane. This replaces the tail of AC1.3

#### AC3.5 Hangup
- **Given** a loop mid-iteration
- **When** `SIGHUP` is delivered
- **Then** no Herdr call is recorded after the signal

---

## Open points from the real run

These need the repository owner's decision. The rules above are unchanged apart from the notes that point here.

1. **I1, the interrupt path.** Either leave it, in which case the `Interrupted` row of R5 is never sent and the notes under R5 and R8 describe what the user sees, or fix it first as a bug of its own. The fix would also stop a headless interrupt from leaving the backend running.
2. **I2, the hang.** R7 sends its two calls after the loop child has exited. Either fix I2 first, or have the TUI parent send them once the TUI has closed without waiting for the child.
3. **I3, the scratchpad.** Phase 3 stores a resume command only once a scratchpad exists. If real runs often have none, the phase does nothing for them. Find out how often before building it.
4. **Hatless display.** With no hats the hat id is `ralph`, so the sidebar line reads `ralph 1/6 ralph`. Leaving the hat id out when it is `ralph` would give `ralph 1/6`.
5. **Popups that say why.** Herdr's own popups read `ralph finished` and `ralph needs attention` (H18). autoloop also calls `herdr notification show` with the question or the stop reason. Doing the same here would put the R5 summary in the popup. Whether Herdr then shows two popups for one event was not tested.
6. **Pane title.** A title shows on the pane border in a split tab (H19). Setting it to `ralph <loop-id>` next to the display name would label split panes. R6 does not set it.

## Testing

- **Unit:** name derivation, the summary and state mapping over every `TerminationReason`, the R1 gate over its inputs, resume argv construction and refusal, POSIX quoting round-trip.
- **Integration:** the fake Herdr with the mock-backend pattern already used in `crates/ralph-cli/tests/integration_run.rs`. Reporting is switched on with `--herdr-pane`, since test stdio is never a terminal.
- **Manual, against a real Herdr:** use a throwaway named session (`herdr --session ralph-herdr-test`), never the default one, and delete it afterwards with `herdr session stop` and `herdr session delete`. Run a real loop there once per phase and compare the sidebar with what `docs/guide/herdr.md` says.
- `cargo test` passes when run from inside a Herdr pane and leaves that pane's agent unchanged.

## Decisions (2026-11-12)

Made by the repository owner. The rules above already reflect them; this records what was turned down.

| Rule | Chosen | Turned down |
|---|---|---|
| R6 | The agent line shows progress (`ralph <summary>`). | Showing the loop name there, with progress only for users who add `state_text` to their sidebar rows. |
| R5 | Ends that need a decision report `blocked`. | Reporting every end as `idle`, which Herdr shows as `done`. |
| R9 | Backends lose all six `HERDR_*` variables. | Removing only `HERDR_PANE_ID`, which would let a hat's agent keep driving Herdr. |
