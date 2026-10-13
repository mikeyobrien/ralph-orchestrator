# Ralph v3: PR FAQ

Status: draft for Mikey's review. 2026-10-13.
Companion to `ralph-v3-design.md`. Mechanics live there. This doc starts from the customer.

---

## Press release

**Ralph v3: hand off the work, watch it move, say yes when it's right.**

*Ralph v3 turns a chat into a workshop. You describe what you want. Ralph picks a workflow, starts the work, and shows each step as it happens. You step in when you want to, not because you have to.*

**Who it's for.** People who build with AI agents every day. A solo developer with three repos. A writer who runs their own site. Someone who has used Ralph v2 and is tired of tuning iteration caps.

**The problem.** Agent loops today need babysitting. You set a cap of 100 iterations and hope. The loop stops early or runs on. You find out by reading logs. When it says "done," you check by hand. To change course, you wait for the next iteration. If the machine restarts, the work is lost or half-done.

**What Ralph v3 does.** You talk to one agent, the strategist. You say what you want. The strategist hands each task to a workflow: plan-build-review for code, write-preview-publish for a site, research for a question. Each task runs on its own, in parallel with the others.

Above the chat, an activity bar shows each task. You see which role it is in: planner, builder, reviewer. You see the tool it is running right now. You see how close it is to done and how much budget it has used.

You can steer any task mid-run. "Use the darker header." The task picks that up after its current step. It acknowledges it before it can finish.

When a task thinks it is done, it says so and shows its evidence: the tests it ran, the files it changed, the URL it deployed. A separate checker reviews that evidence. If something is missing, the task goes back to work. You only hear about it when it is really done, or when it needs you.

Budgets are guardrails, not walls. When a task passes its soft budget, your phone buzzes. You decide: keep going, or stop. Nothing silently gives up.

Ralph v3 runs on a durable agent daemon. Every step is saved before it is shown. If the machine restarts, the work continues where it left off.

Use the agents you already pay for. A role can run on Claude Code, pi, Codex, or Gemini.

**Customer story: a static docs site.** You run a static docs site on Cloudflare Pages. Production is example.com. On your phone, you type: "Add a last-updated line to each page." Ralph starts a site-change task. The activity bar shows *writer*, then the build and test run, then "PR opened." Then *previewer*: the change is deployed as a Cloudflare Pages branch preview. The live site does not change. You get a push with the preview link. You open it. The line is too big. You reply, "Smaller, and gray." The task goes back to *writer*. A few minutes later, a new link. It looks right. You tap **Approve**. Ralph deploys that commit to production. The live site updates. Ralph reports the commit, the preview link, and a check of the live page. Nobody visited the live site with a half-done change.

> "I used to babysit loops. Now I hand off the task and check the preview on my phone." (Illustrative quote, not a real one.)

**How to start.** Run the daemon. Open the chat on web or phone. Say what you want. Your v2 hat YAML imports as a workflow.

---

## Customer FAQ

**What do I talk to?**
One agent, the strategist. It plans, starts tasks, and tells you what happened. You do not talk to each role.

**What is a workflow?**
A named recipe. It lists roles ("hats"), what each role hands off, what must happen before the task can finish, and its budget. Examples: `code-assist`, `research`, `site-change`.

**What is a hat?**
A role the task's agent wears for a while: planner, builder, reviewer. Switching hats is not a new task. It is the same agent, same task, new role. You see each switch in the activity bar.

**Can I run several tasks at once?**
Yes. Each task runs in its own loop. Code tasks get their own checkout. Finished branches land through Ralph's merge queue.

**How do I steer a task?**
Tell the strategist, or pick the task and type. Your message joins that task after its current tool step. It does not cut a tool off midway. The task must acknowledge your guidance before it can finish.

**How do I stop a task?**
Press Cancel on it. Note: a long-running Claude Code job started in the background may keep running after a cancel. That is a known gap.

**How does Ralph know a task is done?**
The task declares done and lists evidence. Ralph checks required steps happened, your guidance was acknowledged, and nothing is left open. Then a checker reviews the evidence. If it finds gaps, the task goes back to work once. If it fails twice, Ralph asks you.

**What stops a runaway task?**
Budgets on cost, time, and handoffs. Past the soft budget, you get a ping and the task keeps going. At the hard ceiling, the task pauses and asks you. A "stuck" detector pings you if it repeats itself.

**Which agents can do the work?**
The daemon's own model, Claude Code, and pi today. Codex and Gemini are new in v3.

**What if my machine restarts?**
The daemon resumes on boot. Every step was saved before it was shown. Tools that are safe to repeat rerun. Others report that they were interrupted, and the task continues from there.

**Where is the v2 web dashboard?**
Gone. The chat and activity bar replace it. `ralph web` and `ralph-api` are removed.

**Can I bring my v2 config?**
Your hat YAML imports. Iteration and cost caps become soft budgets. Per-hat `backend` becomes a runner choice. Some fields do not import, such as per-hat `scratchpad` and hat `timeout`.

**In the docs-site example, is the preview private?**
Not by default. Cloudflare Pages preview deployments are public. Turn on Cloudflare Access for the project's previews if drafts must stay private.

**Can Ralph publish my site without me?**
Not in the `site-change` workflow. The deploy commands are blocked except in the publish role, and that role runs only after your approval.

---

## Internal FAQ

**What is v3 built on?**
The icedrop pi-durable daemon (`icedrop-labs/pi-durable`, `apps/server`). One process owns a SQLite store. Every change is an atomic commit. `harness.resume()` runs on boot (`apps/server/src/daemon.ts:143`). Clients use `POST /commands` and `GET /events`. This matches #386 (`docs/design/v3-on-icedrop.md`).

**What changes from #386?**
Five things. Details in `ralph-v3-design.md` sections 4, 8, 10, 11.
1. Hats run in a loop per task, a side conversation. The root conversation is the strategist. #386 put hats on the root.
2. Completion is `declare_complete` plus gates plus a verifier. #386 completes on a `loop.complete` emit.
3. Caps ping or pause-and-ask. #386 goes quiet at a cap.
4. Hats can name a runner (Claude Code, pi, Codex, Gemini). #386 rejects per-hat `backend`.
5. An activity bar in the icedrop clients. #386 adds no UI.

**What stays from #386?**
The host. No new store or protocol. `configure({ instructions })` per hat switch. The `emit` tool that ends the visit. `onSettled`/`observe`, one step per idle. Daemon-minted `wake:` request ids. Transcript-derived worn hat. No hook that continues a run. Rust keeps worktrees and the merge queue. The dashboard is deleted.

**Why move hats off the root? #386 rejected that.**
The root can wear one hat at a time. Parallel tasks need parallel agents. #386's main objection was that Send and Steer on the open thread would not reach the hat. v3 answers by addressing steers to a task. The cache objection does not apply: inside a loop, a hat switch still changes only `instructions`.

**Why verify after settle instead of in `onYield`?**
#386's reasoning. A hook that continues a run shares the inbox with your Steer. A configure mid-round reaches tool calls already issued. Verifying after settle costs one run. It keeps one routing path. (The pi-durable study suggested `onYield`. Opinion: #386 is right here.)

**What is the verifier?**
Mechanical gates, then a review. Gates: required events seen, guidance acknowledged, open items empty. Review: opinion, a verifier hat in a side with a cheap model and read-only tools. It can reuse the `runEnd` classifier pattern in `packages/agent/src/checks.ts:583-641`.

**What from v2 survives?**
- Topic-tagged handoffs. Now enforced by the `emit` tool. v2 made scope opt-in (`enforce_hat_scope`, `event_loop/mod.rs:2469`).
- `required_events` as a gate (v2 `check_completion_event`, `event_loop/mod.rs:734-837`).
- Guidance ack (v2 `event_loop/mod.rs:762-776`).
- Presets as named bundles (v2 `presets/*.yml`, `presets/index.json`).
- Rust worktrees, merge queue, loop lock.

**What from v2 goes?**
`loop_runner.rs` as the run path. The in-memory `EventBus`. `.ralph/events-*.jsonl` and `ralph emit`. Marker files for stop, restart, suspend. `ralph-api`. The web dashboard and legacy Node server. Hard iteration caps.

**How do Codex and Gemini runners work?**
New `DelegationRunner` implementations beside `claude_code` and `subagent` (`packages/agent/src/delegation.ts`). Each runs the vendor's non-interactive mode and parses the stream. v2 builds argv for both in `ralph-adapters/src/cli_backend.rs`. The exact interfaces are not chosen yet.

**How does the activity bar get its data?**
From `GET /events`. Event types in `packages/protocol/src/events.ts:30-55` cover runs, turns, tool executions, usage, delegations, and approvals. Hat transitions come from the loop transcript. Opinion: the daemon emits one derived `ralph.activity` event per `observe` step, so clients do not re-derive hats.

**What exists on `integration/v3-prerelease` today?**
#386's design doc and `prototypes/v3-hat-seam` (seam test, exit 0 per #386). `crates/ralph-chat-wasm` and `crates/ralph-chat-mock`: a test chat client and a mock of the daemon's two routes. The README says it is not a product UI.

**How does the docs-site example deploy?**
The example is a static docs site on Cloudflare Pages. Production is `https://example.com`. The Pages project is `docs-site`. Details are in `ralph-v3-design.md` section 13.
1. CR created: `previewer` builds the CR commit and runs `wrangler pages deploy dist --project-name docs-site --branch <cr-branch>`. That is a preview deployment. Production is untouched.
2. You review it at `<cr-branch>.docs-site.pages.dev`.
3. After your yes, `publisher` checks out the approved commit and runs `wrangler pages deploy dist --project-name docs-site --branch main`. It then checks `https://example.com`.

**What does the docs-site example need?**
- A Pages project whose production branch is `main`, with build output in `dist`.
- A Cloudflare API token scoped to that Pages project, in the loop's environment.
- A guard that blocks `--branch main` outside the publish role.
- An approval step using the daemon's approvals (`packages/agent/src/approvals.ts`).

**Could the docs site use Workers previews instead?**
Optionally, as a later migration. Moving to Workers static assets means the config uses `"assets": { "directory": "./dist" }` and a `custom_domain` route for `example.com`. Then `wrangler versions upload --preview-alias cr-<PR>` makes the preview, and `wrangler versions deploy <version-id>@100%` promotes it. This example does not need it.

**Risks**
- Private dependency. Ralph is public. pi-durable is private.
- Background `claude_code` survives abort (#386).
- A verifier can accept plausible but wrong evidence.
- One SSE stream per conversation. Many loops mean many streams.
- One provider session per loop. More cold starts than #386's single root.
- Public preview URLs and production bindings shared by previews (the docs-site example).
- `ViewReader` is new code that interprets every entry (#386).

**Open questions**
1. `SideOrigin`: add `loop`, or reuse `manual`?
2. Does the queue keeper fire when a side conversation goes idle? (#386 open question 2.)
3. Where does the task record live?
4. Codex and Gemini: CLI streams or SDKs?
5. Verifier as a visible hat or an invisible decision check?
6. Is `ralph.activity` an acceptable protocol addition?
7. Docs site: when to merge the PR, before or after promote?
8. How do PRs opened outside Ralph trigger a task? A GitHub Action calling MCP `send_message` needs the daemon reachable and authenticated.

**What ships first?**
#386's faux-provider test, moved into one loop. Then two parallel loops. Then completion gates. Then budget pings. Then the activity bar. Then Codex and Gemini. Then `site-change`.
