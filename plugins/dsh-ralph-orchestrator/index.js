import Schema from '@deepseek-ai/schemastery';
import { z } from 'zod';
import { createServer } from 'node:net';
import { mkdtemp, chmod, mkdir, writeFile, readFile, realpath, rm, access } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, join, relative, isAbsolute } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, randomBytes, timingSafeEqual } from 'node:crypto';
import { RalphProcess, runCommand } from './process.js';
import { BUILTINS, executionConfiguration, inspectHats, shortName, iterationTitle } from './configuration.js';
import { collectSecrets, hatBackend, parseAgentModel, redact, resolveAgentSdk, resolveCodexExecutable, runClaudeIteration, runCodexIteration } from './backends.js';

const ROOT = dirname(fileURLToPath(import.meta.url));
const BRIDGE = join(ROOT, 'bridge.js');
const API = '/api/ralph-orchestrator';
const ACTIVE = new Set(['preparing', 'running', 'review', 'stopping', 'restoring']);
const TERMINAL = new Set(['completed', 'exhausted', 'failed', 'stopped', 'interrupted']);
const objectOutput = { schema: { type: 'object', additionalProperties: true }, render: (_args, value) => [{ type: 'text', text: JSON.stringify(value) }] };
const record = z.object({ id: z.string() }).passthrough();
const domainSpec = { name: 'ralph_orchestrator', version: 1, tables: { runs: { valueSchema: record }, configurations: { valueSchema: record } } };
const nonempty = z.string().trim().min(1);
const startFields = { requestId: nonempty.max(100), workspaceId: nonempty, taskName: z.string().max(120).optional(), prompt: nonempty.max(100000), configurationId: z.string().optional(), maxIterations: z.number().int().min(1).max(1000).default(10), permissionPreset: z.string().optional() };
const requestSchema = z.discriminatedUnion('action', [
  z.object({ action: z.literal('state'), runId: nonempty.optional() }),
  z.object({ action: z.literal('start'), ...startFields }),
  z.object({ action: z.literal('design'), ...startFields }),
  z.object({ action: z.literal('revise'), runId: nonempty, feedback: nonempty.max(50000), maxIterations: z.number().int().min(1).max(1000).optional() }),
  z.object({ action: z.literal('restore'), runId: nonempty, yaml: z.string().min(1).max(262144) }),
  z.object({ action: z.literal('accept'), runId: nonempty, revision: z.number().int().positive(), name: nonempty.max(100) }),
  z.object({ action: z.literal('stop'), runId: nonempty }),
]);
const quote = text => `'${String(text).replaceAll("'", "'\\''")}'`;
const textOf = message => (message.content || []).filter(block => block.type === 'text').map(block => block.text).join('\n');
const fail = (message, code = 'RALPH_ERROR') => Object.assign(new Error(message), { code });
function ownedPath(root, target) { const rel = relative(root, target); return rel !== '..' && !rel.startsWith('../') && !isAbsolute(rel); }
function tool(name, description, parameters, execute) { return { name, description, parameters, output: objectOutput, execute }; }

export const Config = Schema.object({
  ralphPath: Schema.string().description('Ralph executable with custom prompt_mode: file support. Empty uses this checkout build, then PATH.').default(''),
  sessionTimeoutMs: Schema.number().min(1000).description('Maximum duration of one managed iteration: a DSH session or one native CLI backend run.').default(600000),
  codexPath: Schema.string().description('Native Codex CLI for hats with backend codex. Empty resolves PATH, then the profile @openai/codex package.').default(''),
});
export const inject = ['agents', 'workspaceRegistry', 'worktrees', 'workingDirectory', 'sessionTitle', 'sessions', 'sessionPersistence', 'storageDomain', 'permissionPresets', 'sandboxPolicy', 'shell', 'agentDefaultModel', 'agentPresets', 'connection', 'tools', 'subagents', 'jobs'];

export class RalphOrchestrator {
  constructor(ctx, config) {
    this.ctx = ctx;
    this.config = config;
    this.live = new Map();
    this.designGates = new Map();
    // A user-supplied restore is single-flight per run: the second caller is
    // refused synchronously before any validation runs or awaits.
    this.restores = new Set();
    this.listeners = new Set();
    this.sockets = new Set();
    this.serial = Promise.resolve();
    this.closing = false;
    // Lazy, cached resolution: native backends resolve only when a hat uses them.
    this.codex = undefined;
    this.agentSdk = undefined;
    this.loadAgentSdk = async () => (await resolveAgentSdk({ profileDir: this.ctx.get?.('profileContext')?.dir })).module;
  }
  async initialize() {
    this.domain = await this.ctx.storageDomain.open(domainSpec);
    this.runs = this.domain.table('runs');
    this.configurations = this.domain.table('configurations');
    for (const builtin of BUILTINS) if (!this.configurations.get(builtin.id)) await this.configurations.put(builtin.id, { ...builtin, hats: inspectHats(builtin.yaml) });
    const localBuild = join(ROOT, '..', '..', 'target', 'debug', 'ralph');
    if (this.config.ralphPath) this.executable = this.config.ralphPath;
    else { try { await access(localBuild); this.executable = localBuild; } catch { this.executable = 'ralph'; } }
    this.ipcDirectory = await mkdtemp(join(tmpdir(), 'dsh-ralph-'));
    await chmod(this.ipcDirectory, 0o700);
    this.socketPath = join(this.ipcDirectory, 'bridge.sock');
    this.server = createServer(socket => this.receiveBridge(socket));
    await new Promise((resolve, reject) => { this.server.once('error', reject); this.server.listen(this.socketPath, resolve); });
    await chmod(this.socketPath, 0o600);
    await this.refreshWorkspaces();
  }
  async refreshWorkspaces() {
    this.workspaceInfo = await Promise.all(this.ctx.workspaceRegistry.list().map(async workspace => {
      const item = { id: workspace.id, title: workspace.title, path: workspace.path };
      try {
        const [{ stdout: dirty }, { stdout: head }] = await Promise.all([
          runCommand('git', ['-C', workspace.path, 'status', '--porcelain'], { signal: AbortSignal.timeout(5000) }),
          runCommand('git', ['-C', workspace.path, 'rev-parse', '--short', 'HEAD'], { signal: AbortSignal.timeout(5000) }),
        ]);
        return { ...item, dirty: Boolean(dirty.trim()), head: head.trim() };
      } catch { return item; } // Non-Git workspaces remain selectable and fail explicitly on Start.
    }));
  }
  state() {
    const catalog = this.ctx.permissionPresets.catalog();
    const known = new Map((this.workspaceInfo || []).map(workspace => [workspace.id, workspace]));
    return {
      workspaces: this.ctx.workspaceRegistry.list().map(workspace => known.get(workspace.id) || { id: workspace.id, title: workspace.title, path: workspace.path }),
      configurations: [...this.configurations.entries()].map(([, value]) => value),
      runs: [...this.runs.entries()].map(([, value]) => value).sort((a, b) => b.createdAt - a.createdAt),
      defaults: { maxIterations: 10, permissionPresets: catalog.options, permissionPreset: catalog.defaultPreset },
    };
  }
  toolState(runId) {
    if (runId) return { run: this.requireRun(runId) };
    const state = this.state();
    return {
      workspaces: state.workspaces,
      configurations: state.configurations.map(({ id, name, revision, hats }) => ({ id, name, revision, hats: (hats || []).map(({ id, name }) => ({ id, name })) })),
      runs: state.runs.slice(0, 20).map(run => ({
        ...Object.fromEntries(['id', 'name', 'mode', 'status', 'iteration', 'maxIterations', 'workspaceId', 'path', 'summaryId', 'configurationId', 'createdAt', 'updatedAt'].filter(key => run[key] !== undefined).map(key => [key, run[key]])),
        sessionCount: (run.sessions || []).length,
        ...(run.error ? { error: run.error.slice(0, 1000) } : {}),
        ...(run.summaryError ? { summaryError: run.summaryError.slice(0, 1000) } : {}),
      })),
      totalRuns: state.runs.length,
      defaults: state.defaults,
    };
  }
  publish() { const state = this.state(); for (const listener of this.listeners) listener(state); }
  async update(id, change) { const value = await this.runs.update(id, current => ({ ...current, ...change(current), updatedAt: Date.now() })); this.publish(); return value; }
  enqueue(operation) {
    const result = this.serial.then(async () => {
      if (this.closing) throw fail('Ralph Orchestrator is shutting down');
      return operation();
    });
    this.serial = result.catch(() => {});
    return result;
  }
  reconcile() { return this.enqueue(() => this.reconcileSummaries()); }
  async reconcileSummaries() {
    // Called only after an authenticated request, when preset composition is ready.
    for (const [id, stored] of this.runs.entries()) {
      if (this.live.has(id)) continue;
      let run = stored;
      // Restart marker: a run that was ACTIVE at DSH restart (preparing/
      // running/restoring) is interrupted. A draft-less review (designer died
      // before proposing) is also interrupted so the user can restore it.
      const needsInterruption = (ACTIVE.has(run.status) && run.status !== 'review')
        || (run.mode === 'design' && run.status === 'review' && !run.draft);
      if (needsInterruption) {
        const interrupted = await this.update(id, () => ({ status: 'interrupted', error: 'DSH restarted during this run. Work was preserved; execution was not replayed.' }));
        if (interrupted) run = interrupted;
      }
      // A surviving review (a design with a draft) is still awaiting the user,
      // not finalizable. Only terminal runs get a summary; recovery must never
      // finalize or resurrect active work.
      if (!TERMINAL.has(run.status)) continue;
      if (!run.path) continue;
      try {
        if (await this.persistedSummary(run)) await this.referenceSummary(id);
        else await this.recoverSummary(id);
      } catch (error) {
        await this.update(id, () => ({ summaryError: error.message }));
        throw error;
      }
    }
  }
  async persistedSummary(run) {
    if (!run.summaryId || !await this.ctx.sessionPersistence.stat(run.summaryId)) return false;
    const reader = await this.ctx.sessionPersistence.open(run.summaryId, 'read');
    try { await reader.read(0, 1); } finally { await reader.close(); }
    return true;
  }
  async referenceSummary(id, change) {
    const run = this.requireRun(id);
    const matches = run.sessions.filter(item => item.id === run.summaryId);
    if (!change && matches.length === 1) return run;
    const entry = { title: 'DONE - Summary', status: 'persisted', ...matches[0], ...change, id: run.summaryId };
    return this.update(id, current => ({ sessions: [...current.sessions.filter(item => item.id !== current.summaryId), entry] }));
  }
  recordedEvidence(run) {
    return { name: run.name, objective: run.prompt, outcome: run.status, error: run.error, path: run.path, branch: run.branch, baseCommit: run.baseCommit, engineOutcome: run.engineOutcome, savedConfigurationId: run.savedConfigurationId, iterations: run.sessions.filter(item => item.id !== run.summaryId).map(item => ({ id: item.id, title: item.title, status: item.status, handoff: item.handoff, error: item.error })), backends: (run.backends || []).map(({ iteration, title, hatId, backend, model, status, code, error, durationMs, outputBytes, logPath }) => ({ iteration, title, hatId, backend, model, status, code, error, durationMs, outputBytes, logPath })) };
  }
  async recoverSummary(id) {
    let run = this.requireRun(id);
    if (!run.summaryId) run = await this.update(id, () => ({ summaryId: randomUUID() }));
    await this.referenceSummary(id, { title: 'DONE - Summary', status: 'recovering' });
    const handle = await this.ctx.agents.create({ sessionId: run.summaryId, meta: { cwd: run.path, agentPreset: 'ralph-summary' }, agentOptions: run.model, setup: async agentCtx => { await this.ctx.agentPresets.mount(agentCtx, 'ralph-summary'); } });
    try {
      this.ctx.permissionPresets.set(handle.agent.session, run.permissionPreset);
      const workspace = this.ctx.workspaceRegistry.get(run.workspaceId);
      if (!workspace) throw fail('Run workspace not found during summary recovery', 'NOT_FOUND');
      await workspace.attachSession(run.summaryId);
      if (this.ctx.sessionTitle.rename(handle.agent.session, 'DONE - Summary').title !== 'DONE - Summary') throw fail('DSH cannot preserve the required summary title');
      const text = `Recorded-evidence summary after interrupted or missing summary materialization.\nOutcome: ${run.status}. DONE means this run ended, not that its objective succeeded.\nNo model was invoked, no iteration was replayed, and no work or verification was performed by this recovery. Handoffs below are recorded claims, not independently verified results.\n\n${JSON.stringify(this.recordedEvidence(run), null, 2)}`;
      handle.agent.session.append('user/message', { id: `ralph-summary-evidence:${run.summaryId}`, role: 'user', source: { kind: 'user' }, content: [{ type: 'text', text }] }, { surfaceOp: 'append' });
      if (!await this.ctx.sessions.flush(handle.agent.session)) throw fail('No DSH session persistence listener participated');
    } finally { await handle.dispose(); }
    await this.referenceSummary(id, { status: 'completed', kind: 'recorded-evidence', error: null });
    await this.update(id, () => ({ summaryError: null }));
  }
  dispatch(input, { user = false, agent } = {}) {
    return this.enqueue(async () => {
      const args = requestSchema.parse(input);
      await this.reconcileSummaries();
      if (args.action === 'state') return user && !args.runId ? this.state() : this.toolState(args.runId);
      if (args.action === 'accept' && !user) throw fail('Only the user can accept a hat configuration', 'USER_ONLY');
      if (agent && args.permissionPreset && args.permissionPreset !== this.ctx.permissionPresets.current(agent.session)) throw fail('Permission changes must be selected by the user', 'USER_ONLY');
      if (args.action === 'start' || args.action === 'design') return this.start(args, agent);
      if (args.action === 'revise') return this.revise(args);
      if (args.action === 'restore') return this.restore(args);
      if (args.action === 'accept') return this.accept(args);
      if (args.action === 'stop') return this.stop(args.runId);
    });
  }
  requireRun(id) { const run = this.runs.get(id); if (!run) throw fail('Run not found', 'NOT_FOUND'); return run; }
  designGate(id, minimumRevision) {
    this.designGates.set(id, Math.max(this.designGates.get(id) || 0, minimumRevision));
    return minimumRevision;
  }
  async validate(yaml, maxIterations = 10, cwd) {
    const prepared = executionConfiguration(yaml, { bridgePath: BRIDGE, maxIterations });
    const directory = await mkdtemp(join(tmpdir(), 'dsh-ralph-validate-'));
    try {
      const path = join(directory, 'ralph.yml');
      await writeFile(path, prepared, { mode: 0o600 });
      await runCommand(this.executable, ['-c', path, 'run', '--dry-run', '--no-tui', '--skip-preflight', '-p', 'Validate hat configuration without execution'], { cwd: directory, signal: AbortSignal.timeout(15000) });
      return { yaml, hats: inspectHats(yaml), prepared };
    } finally { await rm(directory, { recursive: true, force: true }); }
  }
  async start(args, initiatingAgent) {
    const duplicate = [...this.runs.entries()].map(([, run]) => run).find(run => run.requestId === args.requestId);
    if (duplicate) return duplicate;
    const source = this.ctx.workspaceRegistry.get(args.workspaceId);
    if (!source) throw fail('Source workspace not found', 'NOT_FOUND');
    const { stdout: root } = await runCommand('git', ['-C', source.path, 'rev-parse', '--show-toplevel'], { signal: AbortSignal.timeout(10000) });
    const projectName = shortName(basename(root.trim()));
    const taskName = shortName(args.taskName || args.prompt.split(/\s+/).slice(0, 6).join(' '));
    if (!taskName) throw fail('Enter a task short name containing letters or numbers');
    const name = `${projectName}-${taskName}`;
    if ([...this.runs.entries()].some(([, run]) => run.name === name) || this.ctx.workspaceRegistry.list().some(workspace => workspace.title === name)) throw fail('This run name already exists. Choose a different task short name.', 'NAME_CONFLICT');
    const configuration = this.configurations.get(args.configurationId || 'builtin:build-review');
    if (!configuration) throw fail('Hat configuration not found', 'NOT_FOUND');
    const { hats } = await this.validate(configuration.yaml, args.maxIterations, source.path);
    const catalog = this.ctx.permissionPresets.catalog();
    const permissionPreset = initiatingAgent ? this.ctx.permissionPresets.current(initiatingAgent.session) : (args.permissionPreset || catalog.defaultPreset);
    this.ctx.permissionPresets.resolve(permissionPreset);
    const model = this.ctx.agentDefaultModel.currentSelection();
    const run = { id: randomUUID(), requestId: args.requestId, mode: args.action === 'design' ? 'design' : 'run', name, taskName, sourceWorkspaceId: source.id, sourcePath: source.path, prompt: args.prompt, status: 'preparing', maxIterations: args.maxIterations, iteration: 0, sessions: [], drafts: [], configurationId: configuration.id, configurationRevision: configuration.revision, yaml: configuration.yaml, hats, permissionPreset, model, createdAt: Date.now(), updatedAt: Date.now(), summaryId: randomUUID() };
    await this.runs.put(run.id, run);
    this.publish();
    const live = { controller: new AbortController(), handles: new Set(), token: randomBytes(32).toString('hex'), invocations: new Set(), process: null };
    this.live.set(run.id, live);
    live.task = this.launch(run.id, live).catch(error => this.failRun(run.id, error));
    return run;
  }
  async isolate(id, live) {
    const run = this.requireRun(id);
    const setup = await this.ctx.agents.create({ sessionId: randomUUID(), meta: { cwd: run.sourcePath }, signal: live.controller.signal });
    live.handles.add(setup);
    try {
      this.ctx.permissionPresets.set(setup.agent.session, run.permissionPreset);
      this.ctx.sessionTitle.rename(setup.agent.session, `Ralph setup - ${run.name}`);
      const checkout = await this.ctx.worktrees.create(setup.agent, { name: run.name, from: 'HEAD' }, live.controller.signal);
      const workspace = await this.ctx.workspaceRegistry.create(checkout.path, run.name);
      await this.update(id, () => ({ path: checkout.path, branch: checkout.branch, baseCommit: checkout.baseCommit, workspaceId: workspace.id }));
      return checkout.path;
    } finally {
      await this.ctx.sessions.flush(setup.agent.session);
      await this.ctx.workspaceRegistry.archiveSession(setup.agent.id);
      await setup.dispose();
      live.handles.delete(setup);
    }
  }
  async launch(id, live) {
    await this.isolate(id, live);
    live.controller.signal.throwIfAborted();
    if (this.requireRun(id).mode === 'design') return this.generate(id, live);
    const run = this.requireRun(id);
    const directory = join(run.path, '.ralph', 'dsh');
    await mkdir(directory, { recursive: true });
    const configPath = join(directory, 'run.yml');
    const executionYaml = executionConfiguration(run.yaml, { bridgePath: BRIDGE, maxIterations: run.maxIterations });
    await writeFile(configPath, executionYaml, { mode: 0o600 });
    await this.update(id, () => ({ status: 'running', configPath, executionYaml }));
    live.controller.signal.throwIfAborted();
    live.process = new RalphProcess({ executable: this.executable, cwd: run.path,
      args: ['-c', configPath, 'run', '--rpc', '--no-auto-merge', '--skip-preflight', '-p', run.prompt, '--max-iterations', String(run.maxIterations)],
      env: { ...process.env, DSH_RALPH_BRIDGE_SOCKET: this.socketPath, DSH_RALPH_BRIDGE_TOKEN: live.token, DSH_RALPH_RUN_ID: id },
      onEvent: event => { if (event.type === 'iteration_end') live.lastIteration = event.iteration; },
    });
    const outcome = await live.process.finished;
    live.process = null;
    if (live.controller.signal.aborted) return;
    const reason = outcome.terminal?.reason;
    const status = reason === 'completed' && outcome.code === 0 ? 'completed' : reason === 'max_iterations' ? 'exhausted' : 'failed';
    await this.update(id, () => ({ status, engineOutcome: reason || 'disconnected', ...(status === 'failed' ? { error: outcome.error || outcome.stderr || 'Ralph exited without successful terminal evidence' } : {}) }));
    await this.finalize(id, live);
    this.live.delete(id);
  }
  receiveBridge(socket) {
    this.sockets.add(socket);
    socket.setEncoding('utf8');
    let buffer = '', started = false, responded = false;
    const disconnected = new AbortController();
    socket.on('error', () => {});
    socket.on('close', () => { this.sockets.delete(socket); if (!responded) disconnected.abort(); });
    socket.on('data', chunk => {
      if (started) return;
      buffer += chunk.toString('utf8');
      if (Buffer.byteLength(buffer) > 3 * 1024 * 1024) { socket.destroy(); return; }
      const newline = buffer.indexOf('\n');
      if (newline === -1) return;
      started = true;
      let request;
      try { request = JSON.parse(buffer.slice(0, newline)); } catch { socket.destroy(); return; }
      this.bridge(request, disconnected.signal).then(output => ({ ok: true, output }), error => ({ ok: false, error: error.message })).then(response => { responded = true; socket.end(JSON.stringify(response) + '\n'); });
    });
  }
  async bridge(request, disconnected) {
    const live = this.live.get(request.runId);
    const run = this.requireRun(request.runId);
    if (!live || run.status !== 'running' || !live.process || live.controller.signal.aborted) throw fail('This run is not accepting iterations');
    if (typeof request.token !== 'string' || request.token.length !== live.token.length || !timingSafeEqual(Buffer.from(request.token), Buffer.from(live.token))) throw fail('Invalid bridge authorization', 'UNAUTHORIZED');
    if (typeof request.id !== 'string' || live.invocations.has(request.id)) throw fail('Duplicate bridge invocation');
    if (typeof request.prompt !== 'string' || Buffer.byteLength(request.prompt) > 2 * 1024 * 1024) throw fail('Invalid bridge prompt');
    if (live.bridgeInFlight || live.handles.size) throw fail('Overlapping backend invocation is not supported');
    live.bridgeInFlight = true;
    try {
      if (await realpath(request.cwd) !== run.path || request.env?.RALPH_WORKSPACE_ROOT !== run.path || !ownedPath(run.path, request.env?.RALPH_EVENTS_FILE || '')) throw fail('Bridge workspace or events-file mismatch');
      live.invocations.add(request.id);
      const state = await live.process.request('get_state');
      if (!Number.isInteger(state.iteration) || state.iteration < 1 || state.iteration <= run.iteration || state.iteration > run.maxIterations || state.completed) throw fail('Invalid or replayed iteration state');
      const env = { RALPH_BIN: request.env.RALPH_BIN, RALPH_WORKSPACE_ROOT: run.path, RALPH_EVENTS_FILE: request.env.RALPH_EVENTS_FILE, PATH: request.env.PATH };
      const bin = await realpath(env.RALPH_BIN);
      if (isAbsolute(this.executable) && bin !== await realpath(this.executable)) throw fail('Bridge executable mismatch');
      return await this.session(run.id, live, { iteration: state.iteration, hatId: state.hat, hat: state.hat_display || state.hat, prompt: request.prompt, env, invocationId: request.id, signal: AbortSignal.any([live.controller.signal, disconnected]) });
    } finally { live.bridgeInFlight = false; }
  }
  async session(id, live, { iteration, hat, hatId, prompt, env, invocationId, signal = live.controller.signal, summary = false, designer = false }) {
    signal.throwIfAborted();
    const run = this.requireRun(id);
    // Role-management sessions stay in DSH; only work iterations may select a backend.
    const backend = summary || designer ? { type: 'dsh' } : hatBackend(run, hatId ?? hat);
    if (backend.type !== 'dsh') return this.runBackend(id, live, { iteration, hat, backend, prompt, env, signal });
    const sessionId = summary ? run.summaryId : randomUUID();
    const title = summary ? 'DONE - Summary' : iterationTitle(iteration, run.maxIterations, hat, run.taskName.replaceAll('-', ' '));
    if (summary) await this.referenceSummary(id, { title, status: 'running' });
    else await this.update(id, current => ({ iteration, sessions: [...current.sessions, { id: sessionId, title, iteration, hat, status: 'running', ...(invocationId ? { invocationId } : {}) }] }));
    const agentPreset = summary ? 'ralph-summary' : designer ? 'ralph-hat-designer' : 'ralph-worker';
    const agentOptions = backend.model ? parseAgentModel(backend.model) : run.model;
    const handle = await this.ctx.agents.create({ sessionId, meta: { cwd: run.path, agentPreset }, agentOptions, signal,
      setup: async (agentCtx, agent) => {
        await this.ctx.agentPresets.mount(agentCtx, agentPreset);
        // The mounted preset owns role policy, including reopened sessions.
        // This setup adds only the current run's bound operation tools.
        if (!designer && !summary) {
          agentCtx.tools.register(tool('ralph', 'Run Ralph emit, task, or memory commands in this iteration’s exact run context. Use this instead of bash for Ralph commands.', { type: 'object', properties: { args: { type: 'array', items: { type: 'string' }, minItems: 1 } }, required: ['args'], additionalProperties: false }, (args, exec) => this.executeRalph(id, agent, env, args.args, exec.signal)));
        }
        if (!summary) agentCtx.tools.register(tool('ralph_iteration_title', 'Name this iteration using a short description of its current work.', { type: 'object', properties: { name: { type: 'string' } }, required: ['name'], additionalProperties: false }, async args => {
          const next = iterationTitle(iteration, this.requireRun(id).maxIterations, hat, args.name);
          const accepted = this.ctx.sessionTitle.rename(agent.session, next);
          await this.update(id, current => ({ sessions: current.sessions.map(item => item.id === sessionId ? { ...item, title: accepted.title } : item) }));
          return { title: accepted.title };
        }));
        if (designer) {
          const current = this.requireRun(id);
    const minimum = (current.draft?.revision || 0) + 1;
    if (current.status === 'review' && (current.draft?.revision || 0) >= minimum) throw fail('This design revision was superseded; the previous draft is unchanged', 'STALE_REVISION');
    const gate = this.designGate(id, minimum);
          agentCtx.tools.register(tool('ralph_propose_configuration', 'Submit a complete Ralph YAML configuration for human review. This does not accept or save it.', { type: 'object', properties: { yaml: { type: 'string' } }, required: ['yaml'], additionalProperties: false }, async args => {
            const current = this.requireRun(id);
            if (this.designGates.get(id) !== gate || (current.draft?.revision || 0) >= gate) throw fail('This designer revision is superseded; its draft was not recorded', 'STALE_REVISION');
            const candidate = await this.validate(args.yaml, current.maxIterations, run.path);
            live.proposal = { yaml: candidate.yaml, hats: candidate.hats };
            return { valid: true, hats: candidate.hats.map(item => ({ id: item.id, name: item.name })) };
          }));
        }
      },
    });
    live.handles.add(handle);
    const agent = handle.agent;
    let off, timer, cancel;
    try {
      const composed = this.ctx.agentPresets.composedPreset(agent.ctx);
      if (composed !== agentPreset) throw fail('The managed session did not mount its required role preset');
      const schemas = this.ctx.tools.schemas(agent);
      await this.update(id, current => ({ sessions: current.sessions.map(item => item.id === sessionId ? { ...item, agentPreset: composed, cwd: agent.session.header.cwd, isSeeded: agent.session.header.isSeeded, toolNames: schemas.map(schema => schema.name), toolSchemaBytes: Buffer.byteLength(JSON.stringify(schemas)) } : item) }));
      this.ctx.permissionPresets.set(agent.session, run.permissionPreset);
      await this.ctx.workspaceRegistry.get(run.workspaceId).attachSession(sessionId);
      const pinned = this.ctx.sessionTitle.rename(agent.session, title);
      if (pinned.title !== title) throw fail('DSH title length configuration cannot preserve the required iteration name');
      const completion = new Promise((resolve, reject) => {
        let admitted = false;
        off = this.ctx.on('session/event', (session, event) => {
          if (session.id !== sessionId) return;
          if (event.type === 'user/message' && event.data.id === messageId) admitted = true;
          if (event.type === 'turn/end' && admitted) resolve(event.data.reason);
        });
        cancel = () => { agent.cancel({ kind: 'user' }); reject(fail('Iteration cancelled', 'ABORTED')); };
        signal.addEventListener('abort', cancel, { once: true });
        if (signal.aborted) cancel();
        timer = setTimeout(() => { agent.cancel({ kind: 'hook', reason: 'Ralph session time limit' }); reject(fail('DSH iteration exceeded its session time limit', 'TIMEOUT')); }, this.config.sessionTimeoutMs);
      });
      const messageId = randomUUID();
      const instructions = summary || designer ? '' : '\n\nDSH integration: use the scoped ralph tool for all Ralph CLI commands so events, tasks and memories use this exact run. Call ralph_iteration_title with a short generated name for this iteration. The final task outcome is decided by Ralph, not a DSH goal.';
      if (!signal.aborted) {
        if (instructions) agent.inject({ id: randomUUID(), role: 'user', source: { kind: 'user' }, content: [{ type: 'text', text: instructions.trim() }] });
        agent.followup({ id: messageId, role: 'user', source: { kind: 'user' }, content: [{ type: 'text', text: prompt }] });
      }
      const reason = await completion;
      await agent.whenIdle();
      if (!await this.ctx.sessions.flush(agent.session)) throw fail('No DSH session persistence listener participated');
      const messages = agent.session.ownEvents().filter(event => event.type === 'assistant/message').map(event => textOf(event.data.message));
      const output = messages.join('\n');
      if (reason.kind !== 'completed') throw fail(`DSH iteration ended with ${reason.kind}${reason.error?.message ? ': ' + reason.error.message : ''}`);
      await this.update(id, current => ({ sessions: current.sessions.map(item => item.id === sessionId ? { ...item, status: 'completed', handoff: output.slice(-6000) } : item) }));
      return output;
    } catch (error) {
      await this.update(id, current => ({ sessions: current.sessions.map(item => item.id === sessionId ? { ...item, status: 'failed', error: error.message } : item) }));
      throw error;
    } finally {
      off?.(); clearTimeout(timer); if (cancel) signal.removeEventListener('abort', cancel);
      agent.cancel({ kind: 'disposed' });
      await this.ctx.subagents.drainDescendants([agent]);
      for (const job of this.ctx.jobs.list(agent.id)) if (job.status === 'running') this.ctx.jobs.kill(job.id, agent.id, 'Ralph iteration ended');
      await agent.whenIdle();
      await this.ctx.sessions.flush(agent.session);
      await handle.dispose();
      live.handles.delete(handle);
    }
  }
  /**
   * Run one work iteration on a native CLI backend inside the run worktree.
   * These iterations own a child process or SDK query, not a DSH session; the
   * backend enforces the timeout, kills on abandon, and captures its own log.
   */
  async runBackend(id, live, { iteration, hat, backend, prompt, env, signal }) {
    const run = this.requireRun(id);
    if (run.status !== 'running' || !live.process || live.controller.signal.aborted) throw fail('This run is not accepting iterations');
    signal.throwIfAborted();
    const directory = join(run.path, '.ralph', 'dsh', 'backends');
    await mkdir(directory, { recursive: true });
    const logPath = join(directory, `${String(iteration).padStart(3, '0')}-${shortName(backend.hatId || hat) || 'hat'}-${backend.type}.log`);
    const entry = { id: randomUUID(), iteration, hatId: backend.hatId, title: iterationTitle(iteration, run.maxIterations, hat, run.taskName.replaceAll('-', ' ')), backend: backend.type, ...(backend.model ? { model: backend.model } : {}), status: 'running', startedAt: Date.now(), logPath };
    await this.update(id, current => ({ iteration, backends: [...(current.backends || []), entry] }));
    const childEnv = { ...process.env };
    for (const key of ['RALPH_BIN', 'RALPH_WORKSPACE_ROOT', 'RALPH_EVENTS_FILE', 'PATH']) if (env?.[key] !== undefined) childEnv[key] = env[key];
    const timeoutMs = this.config.sessionTimeoutMs;
    let result;
    try {
      if (backend.type === 'codex') {
        this.codex ??= resolveCodexExecutable({ codexPath: this.config.codexPath, profileDir: this.ctx.get?.('profileContext')?.dir });
        result = await runCodexIteration({ ...(await this.codex), cwd: run.path, prompt, model: backend.model, env: childEnv, signal, timeoutMs, logPath });
      } else if (backend.type === 'claude-code') {
        this.agentSdk ??= this.loadAgentSdk();
        result = await runClaudeIteration({ sdk: await this.agentSdk, cwd: run.path, prompt, model: backend.model, env: childEnv, signal, timeoutMs, logPath });
      } else throw fail(`Unsupported hat backend ${backend.type}`);
    } catch (error) {
      const code = error.code || 'BACKEND_FAILED';
      const message = redact(error.message, collectSecrets(childEnv));
      await this.update(id, current => ({ backends: (current.backends || []).map(item => item.id === entry.id ? { ...item, status: 'failed', code, error: message, durationMs: Date.now() - entry.startedAt } : item) }));
      throw error;
    }
    await this.update(id, current => ({ backends: (current.backends || []).map(item => item.id === entry.id ? { ...item, status: 'completed', durationMs: result.durationMs, outputBytes: Buffer.byteLength(result.output), handoff: result.output.slice(-6000), ...(result.exitCode !== undefined ? { exitCode: result.exitCode } : {}) } : item) }));
    return result.output;
  }
  async executeRalph(id, agent, env, args, signal) {
    const run = this.requireRun(id);
    if (run.status !== 'running' || !Array.isArray(args) || !args.length || args.some(arg => typeof arg !== 'string')) throw fail('Invalid or inactive Ralph command');
    if (!['emit', 'tools', 'task', '--help'].includes(args[0])) throw fail('Only Ralph emit, task, and memory tooling is available in an iteration');
    const command = [env.RALPH_BIN, '-c', run.configPath, ...args].map(quote).join(' ');
    const result = await (await this.ctx.shell.execute(this.ctx.shell.resolve({ command, workdir: run.path, env, signal, timeoutMs: 30000, sandboxPolicy: this.ctx.sandboxPolicy.resolve({ session: agent.session }) }))).result();
    if (result.exitCode !== 0) throw fail(result.stderr.text || result.stdout.text || `Ralph command exited ${result.exitCode}`);
    return { exitCode: result.exitCode, output: result.stdout.text, ...(result.stderr.text ? { warnings: result.stderr.text } : {}) };
  }
  async generate(id, live, feedback = '') {
    const run = this.requireRun(id);
    const minimum = (run.draft?.revision || 0) + 1;
    const gate = this.designGate(id, minimum);
    live.proposal = null;
    // The designer owns a fresh design (preparing) or the revision the caller
    // already moved out of review (revise sets running). Every other state is
    // left untouched: a terminal run is never resurrected and a newer draft
    // cannot be overwritten by a stale designer.
    const taken = await this.update(id, stored => (stored.status === 'preparing' || stored.status === 'running')
      ? { status: 'running', error: null }
      : {});
    const current = this.requireRun(id);
    if (current.status !== 'running' || taken.status !== 'running') throw fail('This design revision was superseded; the previous draft is unchanged', 'STALE_REVISION');
    if (current.iteration >= current.maxIterations) throw fail('Increase the design budget to create another revision');
    const prompt = `Design a native Ralph hat configuration for the following workflow. This is a human review draft, not permission to run the task.\n\n${current.prompt}\n\nUse Ralph YAML with event_loop settings and hats keyed by ID (name, nonblank description, triggers, publishes, instructions). task.start/task.resume belong to the Ralph coordinator. Preserve native event semantics, include concrete verification gates, and use sequential execution. A hat may set backend: dsh (the default), codex, or claude-code, with an optional model; do not add waves, aggregation, or cost budgets. Submit the complete YAML using ralph_propose_configuration; validation failures can be corrected before the turn ends. Use ralph_iteration_title to name this revision.\n\nExisting configuration:\n${current.draft?.yaml || current.yaml}\n${feedback ? `\nHuman feedback:\n${feedback}` : ''}`;
    try {
      await this.session(id, live, { iteration: current.iteration + 1, hat: 'Hat Designer', prompt, designer: true });
      const storedRun = this.requireRun(id);
      if (!live.proposal) throw fail('The designer did not submit a validated configuration. The previous draft is unchanged.');
      const expected = (storedRun.draft?.revision || 0) + 1;
      if (live.controller.signal.aborted || this.designGates.get(id) !== gate || storedRun.status === 'review' && storedRun.draft?.revision !== expected) throw fail('This design revision was superseded; the previous draft is unchanged', 'STALE_REVISION');
      const draft = { ...live.proposal, revision: expected, feedback };
      // Commit only while this generation still owns the run; a stopped or
      // superseded run is left untouched and a newer draft is never overwritten.
      await this.update(id, stored => (stored.status !== 'running' || this.designGates.get(id) !== gate || (stored.draft?.revision || 0) >= expected
        ? {}
        : { status: 'review', draft, drafts: [...stored.drafts, draft], error: null }));
    } catch (error) {
      if (live.controller.signal.aborted) return;
      // Only the generation that still owns the run may annotate its failure.
      // A stale designer never demotes a newer running state or a terminal run.
      await this.update(id, stored => {
        if (this.designGates.get(id) !== gate) return {};
        if (stored.status === 'running') return { status: 'review', error: error.message };
        if (stored.status === 'review') return { error: error.message };
        return {};
      });
    } finally {
      if (this.designGates.get(id) === gate) this.designGates.delete(id);
    }
  }
  async revise(args) {
    const run = this.requireRun(args.runId);
    if (run.mode !== 'design' || run.status !== 'review') throw fail('Only a design awaiting review can be revised');
    const maximum = args.maxIterations ?? run.maxIterations;
    if (maximum <= run.iteration) throw fail('Increase the design budget before another revision');
    await this.update(run.id, () => ({ maxIterations: maximum, status: 'running' }));
    let live = this.live.get(run.id);
    if (!live) { live = { controller: new AbortController(), handles: new Set(), token: randomBytes(32).toString('hex'), invocations: new Set() }; this.live.set(run.id, live); }
    live.task = this.generate(run.id, live, args.feedback).catch(error => this.failRun(run.id, error));
    return this.requireRun(run.id);
  }
  async restore(args) {
    const run = this.requireRun(args.runId);
    // Single-flight per run: reserve synchronously, before any status change or
    // validation await, so a second restore cannot validate against the same
    // base revision.
    if (this.restores.has(run.id)) throw fail('A restore is already in progress for this design', 'BUSY');
    const supported = run.mode === 'design'
      && (run.status === 'interrupted' || run.status === 'failed' || (run.status === 'review' && !run.draft));
    if (!supported) throw fail('Only an interrupted, failed, or draft-less reviewing design can restore a supplied draft');
    this.restores.add(run.id);
    const priorStatus = run.status;
    const baseRevision = run.draft?.revision || 0;
    const gate = this.designGate(run.id, baseRevision + 1);
    try {
      await this.update(run.id, () => ({ status: 'restoring', error: null }));
      let candidate;
      try {
        candidate = await this.validate(args.yaml, run.maxIterations, run.path);
      } catch (error) {
        const current = this.requireRun(run.id);
        // Invalid input restores exactly the state the user saw, never a
        // different status inferred from draft presence.
        if (this.designGates.get(run.id) === gate && current.status === 'restoring') {
          await this.update(run.id, () => ({ status: priorStatus, error: `Restore validation failed: ${error.message}` }));
        }
        throw error;
      }
      const current = this.requireRun(run.id);
      // Recheck before committing: the run must still be this restore in flight
      // and no newer draft may have landed while validation awaited.
      if (this.designGates.get(run.id) !== gate || current.status !== 'restoring' || (current.draft?.revision || 0) > baseRevision) {
        return current;
      }
      const revision = baseRevision + 1;
      const draft = { yaml: candidate.yaml, hats: candidate.hats, revision, source: 'user-supplied', feedback: 'Restored from the user-supplied configuration; it has not been reviewed or accepted.' };
      await this.update(run.id, stored => (stored.status !== 'restoring' || this.designGates.get(run.id) !== gate || (stored.draft?.revision || 0) > baseRevision
        ? {}
        : { status: 'review', draft, drafts: [...stored.drafts, draft], error: null }));
      return this.requireRun(run.id);
    } finally {
      this.restores.delete(run.id);
      if (this.designGates.get(run.id) === gate) this.designGates.delete(run.id);
    }
  }
  async accept(args) {
    const run = this.requireRun(args.runId);
    if (run.mode !== 'design' || run.status !== 'review' || run.draft?.revision !== args.revision) throw fail('This draft is no longer the current review revision', 'STALE_REVISION');
    await this.validate(run.draft.yaml, run.maxIterations, run.path);
    const configurationId = run.savedConfigurationId || `configuration:${run.id}`;
    const configuration = { id: configurationId, name: args.name, revision: 1, yaml: run.draft.yaml, hats: run.draft.hats, createdAt: Date.now() };
    await this.configurations.put(configurationId, configuration);
    await this.update(run.id, () => ({ status: 'completed', savedConfigurationId: configurationId }));
    const live = this.live.get(run.id) || { controller: new AbortController(), handles: new Set() };
    live.task = this.finalize(run.id, live).catch(error => this.update(run.id, () => ({ error: `Configuration saved; summary failed: ${error.message}` })));
    this.live.set(run.id, live);
    live.task.then(() => this.live.delete(run.id), () => this.live.delete(run.id));
    return this.requireRun(run.id);
  }
  async stop(id) {
    const run = this.requireRun(id);
    if (!ACTIVE.has(run.status)) return run;
    await this.update(id, () => ({ status: 'stopping' }));
    const live = this.live.get(id);
    if (live) {
      live.controller.abort();
      for (const handle of live.handles) handle.agent.cancel({ kind: 'user' });
      await live.process?.stop();
      await live.task;
      for (const handle of [...live.handles]) { await handle.agent.whenIdle(); await handle.dispose(); live.handles.delete(handle); }
    }
    await this.update(id, () => ({ status: 'stopped' }));
    const summaryLive = { controller: new AbortController(), handles: new Set() };
    this.live.set(id, summaryLive);
    summaryLive.task = this.finalize(id, summaryLive).catch(error => this.update(id, () => ({ summaryError: error.message })));
    const release = () => { if (this.live.get(id) === summaryLive) this.live.delete(id); };
    summaryLive.task.then(release, release);
    return this.requireRun(id);
  }
  async failRun(id, error) {
    const run = this.requireRun(id);
    const live = this.live.get(id);
    if (live?.controller.signal.aborted) return;
    // A design that recorded its draft is already in review; its error is
    // annotated but the run is never demoted to failed. Work runs go to
    // failed as before.
    const isDesign = run.mode === 'design';
    await this.update(id, stored => {
      if (isDesign && stored.status === 'review') return { error: error.message };
      return { status: 'failed', error: error.message };
    });
    if (run.path && live && !this.closing) await this.finalize(id, live).catch(summaryError => this.update(id, () => ({ error: `${error.message}; summary failed: ${summaryError.message}` })));
    this.live.delete(id);
  }
  async finalize(id, live) {
    const run = this.requireRun(id);
    if (!run.path) return;
    if (await this.persistedSummary(run)) { await this.referenceSummary(id); return; }
    const evidence = this.recordedEvidence(run);
    const prompt = `Produce the final run summary from these recorded facts. DONE means the run ended, not that the task succeeded. Distinguish verified checks from claims and remaining work. Include iteration session IDs and worktree/branch. Do not modify files, run work, or invent evidence. If this generation fails, these facts remain the readable fallback.\n\n${JSON.stringify(evidence, null, 2)}`;
    await this.session(id, live, { hat: 'Summary', prompt, summary: true });
  }
  async close() {
    this.closing = true;
    // Recovery owns no model task, but its handle must finish disposal before storage closes.
    await this.serial;
    for (const [id, live] of this.live) {
      live.controller.abort();
      for (const handle of live.handles) handle.agent.cancel({ kind: 'disposed' });
      await live.process?.stop();
      await live.task;
      if (ACTIVE.has(this.requireRun(id).status)) await this.update(id, () => ({ status: 'interrupted', error: 'Plugin stopped; work was preserved.' }));
    }
    this.live.clear();
    for (const socket of this.sockets) socket.destroy();
    await new Promise(resolve => this.server.close(resolve));
    await rm(this.ipcDirectory, { recursive: true, force: true });
    this.listeners.clear();
    await this.domain.close();
  }
}
export async function apply(ctx, config) {
  const service = new RalphOrchestrator(ctx, config);
  await service.initialize();
  ctx.effect(() => () => service.close());
  ctx.provide('ralphOrchestrator', service);
  ctx.connection.fetch.register({ path: API, methods: ['GET', 'POST'], requestBody: 'buffered', fetch: async request => {
    try {
      if (request.method === 'GET') { await service.reconcile(); await service.refreshWorkspaces(); return Response.json({ ok: true, value: service.state() }); }
      const text = await request.text();
      if (Buffer.byteLength(text) > 300000) throw fail('Request is too large');
      const value = await service.dispatch(JSON.parse(text), { user: true });
      return Response.json({ ok: true, value });
    } catch (error) { return Response.json({ ok: false, error: { code: error.code || 'RALPH_ERROR', message: error.message } }, { status: error.code === 'NOT_FOUND' ? 404 : 400 }); }
  } });
  ctx.connection.fetch.register({ path: `${API}/events`, methods: ['GET'], requestBody: 'buffered', fetch: async request => {
    try { await service.reconcile(); }
    catch (error) { return Response.json({ ok: false, error: { code: error.code || 'RALPH_ERROR', message: error.message } }, { status: 400 }); }
    let unsubscribe;
    const stream = new ReadableStream({
      start(controller) {
        const send = state => { try { controller.enqueue(new TextEncoder().encode(`event: state\ndata: ${JSON.stringify(state)}\n\n`)); } catch { unsubscribe?.(); } };
        const listener = state => send(state);
        unsubscribe = () => service.listeners.delete(listener);
        service.listeners.add(listener);
        send(service.state());
        request.signal.addEventListener('abort', () => { unsubscribe(); try { controller.close(); } catch {} }, { once: true });
      },
      cancel() { unsubscribe?.(); },
    });
    return new Response(stream, { headers: { 'content-type': 'text/event-stream', 'cache-control': 'no-cache', 'x-accel-buffering': 'no' } });
  } });
  ctx.tools.register(tool('ralph_orchestrator', 'List recent Ralph runs and configuration metadata, inspect a run with state and runId, start isolated runs or hat-design drafts, revise drafts, restore a user-supplied draft, or stop runs. Configuration acceptance stays with the user.', {
    type: 'object', properties: { action: { type: 'string', enum: ['state', 'start', 'design', 'revise', 'restore', 'stop'] }, requestId: { type: 'string', description: 'Stable unique request ID for an idempotent new run.' }, workspaceId: { type: 'string' }, taskName: { type: 'string' }, prompt: { type: 'string' }, configurationId: { type: 'string' }, maxIterations: { type: 'integer', minimum: 1, maximum: 1000 }, runId: { type: 'string' }, feedback: { type: 'string' }, yaml: { type: 'string', description: 'Complete YAML configuration to restore for an interrupted, failed, or draft-less reviewing design.' } }, required: ['action'], additionalProperties: false,
  }, (args, exec) => service.dispatch(args, { agent: exec.agent })));
}
