import test from 'node:test';
import assert from 'node:assert/strict';
import { chmod, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { RalphOrchestrator } from '../index.js';
import { backendError } from '../backends.js';

const fakeCodex = fileURLToPath(new URL('./fixtures/fake-codex-app-server.mjs', import.meta.url));
await chmod(fakeCodex, 0o755);
const prompt = 'Exact native Ralph assembled prompt λ🦀\n\n<hat>Keep this byte-for-byte</hat>';

function claudeSdk(behaviour = {}) {
  const calls = [];
  return {
    calls,
    query({ prompt: text, options }) {
      calls.push({ prompt: text, options });
      if (behaviour.hang) return (async function* () {
        await new Promise((resolve, reject) => options.abortController.signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true }));
      })();
      return (async function* () {
        yield { type: 'assistant', message: { content: [{ type: 'text', text: 'notes' }] } };
        yield { type: 'result', subtype: 'success', is_error: false, result: 'fake claude final answer' };
      })();
    },
  };
}

/**
 * Minimal Host fixture: the DSH agent factory is a strict tripwire for native
 * backend iterations, and the legacy bridge is exercised only when a test opts in.
 */
function fixture({ hats = [], model = { provider: 'test', model: 'test' }, runPath = '/worktree', sdk, codexPath = '' } = {}) {
  const calls = { creations: [], activations: [], backends: [], queries: [] };
  let record = { id: 'run-one', mode: 'run', name: 'project-task', taskName: 'task', status: 'running', prompt: 'Objective', path: runPath, workspaceId: 'workspace-one', iteration: 0, maxIterations: 4, permissionPreset: 'workspace', model, hats, sessions: [], drafts: [], createdAt: 1 };
  const listeners = new Set();
  const bound = new WeakMap();
  const ctx = {
    get: name => name === 'profileContext' ? { name: 'web', dir: '/profile' } : undefined,
    agents: {
      async create(options) {
        calls.creations.push({ sessionId: options.sessionId, meta: structuredClone(options.meta), agentOptions: structuredClone(options.agentOptions) });
        const log = [];
        const definitions = new Map();
        const agentCtx = { definitions, tools: { register(definition) { definitions.set(definition.name, definition); } } };
        const session = {
          id: options.sessionId, header: { id: options.sessionId, cwd: options.meta.cwd, agentPreset: options.meta.agentPreset, isSeeded: false }, ownEvents: () => log,
          append(type, data, surface) { const event = { type, seq: log.length, data: structuredClone(data), ...(surface || {}) }; log.push(event); for (const listener of listeners) listener(session, event); return event; },
        };
        const agent = {
          id: session.id, ctx: agentCtx, session,
          cancel() {}, async whenIdle() {}, inject() {},
          followup(message) { session.append('user/message', message, { surfaceOp: 'append' }); session.append('assistant/message', { message: { content: [{ type: 'text', text: 'external handoff' }] } }); session.append('turn/end', { reason: { kind: 'completed' } }); },
        };
        await options.setup?.(agentCtx, agent);
        return { agent, async dispose() {} };
      },
    },
    agentPresets: { async mount(agentCtx, id) { bound.set(agentCtx, id); }, composedPreset: agentCtx => bound.get(agentCtx) },
    tools: { schemas: () => [] },
    permissionPresets: { catalog: () => ({ options: [], defaultPreset: 'workspace' }), set() {} },
    workspaceRegistry: { list: () => [], get: () => ({ async attachSession() {} }) },
    sessionTitle: { rename: (session, title) => ({ title }) },
    sessions: { async flush() { return true; } },
    on: (name, listener) => { assert.equal(name, 'session/event'); listeners.add(listener); return () => listeners.delete(listener); },
    subagents: { async drainDescendants() {}, async startActivation() { calls.activations.push('startActivation'); throw new Error('DSH subagent providers must never be used as an execution backend'); } },
    jobs: { list: () => [] },
  };
  const service = new RalphOrchestrator(ctx, { sessionTimeoutMs: 5000, codexPath });
  if (sdk) service.loadAgentSdk = async () => sdk;
  service.runs = {
    get: id => id === record.id ? record : undefined,
    entries: () => new Map([[record.id, record]]).entries(),
    async update(id, transform) { record = structuredClone(transform(record)); return record; },
  };
  service.configurations = { entries: () => new Map().entries() };
  const live = { controller: new AbortController(), handles: new Set(), process: { request: async () => ({}) } };
  const env = { RALPH_BIN: '/exact/ralph', RALPH_WORKSPACE_ROOT: runPath, RALPH_EVENTS_FILE: join(runPath, '.ralph', 'events.jsonl'), PATH: process.env.PATH };
  return { service, calls, live, env, record: () => record };
}

async function workspace(t) {
  const path = await mkdtemp(join(tmpdir(), 'ralph-dispatch-'));
  t.after(() => rm(path, { recursive: true, force: true }));
  return path;
}

test('a hat without a backend keeps the exact DSH bridge path and record shape', async t => {
  const runPath = await workspace(t);
  const setup = fixture({ hats: [{ id: 'builder', name: 'Builder' }], runPath });
  const output = await setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Builder', hatId: 'builder', prompt, env: setup.env });
  assert.equal(output, 'external handoff');
  assert.equal(setup.calls.creations.length, 1);
  assert.deepEqual(setup.calls.creations[0].agentOptions, { provider: 'test', model: 'test' }, 'unset hats keep the run-wide model object');
  assert.equal(setup.calls.activations.length, 0);
  assert.equal(setup.record().backends, undefined, 'the legacy path adds no backend bookkeeping');
  assert.equal(setup.record().sessions.length, 1);
  assert.equal(setup.record().sessions[0].backend, undefined);
  assert.equal(setup.record().sessions[0].status, 'completed');
});

test('a per-hat dsh model selects provider/model for that hat only', async t => {
  const runPath = await workspace(t);
  const setup = fixture({ hats: [{ id: 'builder', name: 'Builder', backend: 'dsh', model: 'sparks/GLM-5.3-Flash-EXL3' }, { id: 'reviewer', name: 'Reviewer' }], runPath });
  await setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Builder', hatId: 'builder', prompt, env: setup.env });
  await setup.service.session('run-one', setup.live, { iteration: 2, hat: 'Reviewer', hatId: 'reviewer', prompt, env: setup.env });
  assert.deepEqual(setup.calls.creations[0].agentOptions, { provider: 'sparks', model: 'GLM-5.3-Flash-EXL3' });
  assert.deepEqual(setup.calls.creations[1].agentOptions, { provider: 'test', model: 'test' });
});

test('a codex hat runs through the native app-server, not a DSH session or subagent provider', async t => {
  const runPath = await workspace(t);
  const trace = join(runPath, 'trace.jsonl');
  process.env.FAKE_CODEX_SCENARIO = 'ok';
  process.env.FAKE_CODEX_TRACE = trace;
  t.after(() => { delete process.env.FAKE_CODEX_SCENARIO; delete process.env.FAKE_CODEX_TRACE; });
  const setup = fixture({ hats: [{ id: 'reviewer', name: 'Reviewer', backend: 'codex', model: 'gpt-6.1-sol' }], runPath, codexPath: fakeCodex });
  const output = await setup.service.session('run-one', setup.live, { iteration: 2, hat: 'Reviewer', hatId: 'reviewer', prompt, env: setup.env });
  assert.equal(output, 'fake codex final answer');
  assert.equal(setup.calls.creations.length, 0, 'a native backend must not create a DSH session');
  assert.equal(setup.calls.activations.length, 0, 'no DSH subagent provider may be used');
  const record = setup.record();
  assert.equal(record.iteration, 2);
  assert.equal(record.backends.length, 1);
  const entry = record.backends[0];
  assert.equal(entry.backend, 'codex');
  assert.equal(entry.model, 'gpt-6.1-sol');
  assert.equal(entry.status, 'completed');
  assert.equal(entry.iteration, 2);
  assert.equal(entry.handoff, 'fake codex final answer');
  assert.ok(entry.durationMs >= 0);
  assert.match(entry.logPath, /\.ralph\/dsh\/backends\/002-reviewer-codex\.log$/);
  const log = await readFile(entry.logPath, 'utf8');
  assert.ok(log.includes(prompt));
  assert.match(log, /backend: codex/);
  const spawn = (await readFile(trace, 'utf8')).trim().split('\n').map(line => JSON.parse(line)).find(event => event.kind === 'spawn');
  assert.equal(spawn.cwd, runPath);
  assert.equal(spawn.env.RALPH_BIN, '/exact/ralph');
  assert.match(spawn.env.RALPH_EVENTS_FILE, /\.ralph\/events\.jsonl$/);
});

test('a claude-code hat runs through the Agent SDK with the hat model', async t => {
  const runPath = await workspace(t);
  const sdk = claudeSdk();
  const setup = fixture({ hats: [{ id: 'planner', name: 'Planner', backend: 'claude-code', model: 'claude-opus-5-5' }], runPath, sdk });
  const output = await setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Planner', hatId: 'planner', prompt, env: setup.env });
  assert.equal(output, 'fake claude final answer');
  assert.equal(setup.calls.creations.length, 0);
  assert.equal(setup.calls.activations.length, 0);
  assert.equal(sdk.calls.length, 1);
  assert.equal(sdk.calls[0].prompt, prompt);
  assert.equal(sdk.calls[0].options.cwd, runPath);
  assert.equal(sdk.calls[0].options.model, 'claude-opus-5-5');
  const entry = setup.record().backends[0];
  assert.equal(entry.backend, 'claude-code');
  assert.equal(entry.status, 'completed');
  assert.match(await readFile(entry.logPath, 'utf8'), /backend: claude-code/);
});

test('a failing native backend fails the iteration closed with no DSH fallback', async t => {
  const runPath = await workspace(t);
  process.env.FAKE_CODEX_SCENARIO = 'error';
  process.env.FAKE_CODEX_TRACE = join(runPath, 'trace.jsonl');
  t.after(() => { delete process.env.FAKE_CODEX_SCENARIO; delete process.env.FAKE_CODEX_TRACE; });
  const setup = fixture({ hats: [{ id: 'reviewer', name: 'Reviewer', backend: 'codex', model: 'gpt-6.1-sol' }], runPath, codexPath: fakeCodex });
  await assert.rejects(setup.service.session('run-one', setup.live, { iteration: 2, hat: 'Reviewer', hatId: 'reviewer', prompt, env: setup.env }), /status failed/);
  assert.equal(setup.calls.creations.length, 0, 'failure must never fall back to a DSH session');
  assert.equal(setup.calls.activations.length, 0);
  const entry = setup.record().backends[0];
  assert.equal(entry.status, 'failed');
  assert.equal(entry.code, 'TURN_FAILED');
  assert.match(entry.error, /status failed/);
});

test('an unresolvable Agent SDK fails with the exact resolution failure and no fallback', async t => {
  const runPath = await workspace(t);
  const setup = fixture({ hats: [{ id: 'planner', name: 'Planner', backend: 'claude-code', model: 'claude-opus-5-5' }], runPath });
  const failure = backendError('BACKEND_UNAVAILABLE', 'claude-code backend unavailable: cannot resolve @anthropic-ai/claude-agent-sdk (plugin tree: MODULE_NOT_FOUND; profile node_modules (/profile): MODULE_NOT_FOUND)');
  setup.service.loadAgentSdk = async () => { throw failure; };
  await assert.rejects(setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Planner', hatId: 'planner', prompt, env: setup.env }), error => {
    assert.equal(error, failure);
    assert.match(error.message, /MODULE_NOT_FOUND/);
    return true;
  });
  assert.equal(setup.calls.creations.length, 0);
  assert.equal(setup.record().backends[0].status, 'failed');
  assert.equal(setup.record().backends[0].code, 'BACKEND_UNAVAILABLE');
});

test('summary and designer role sessions stay in DSH even when the hat selects a native backend', async t => {
  const runPath = await workspace(t);
  const setup = fixture({ hats: [{ id: 'builder', name: 'Builder', backend: 'codex', model: 'gpt-6.1-sol' }], runPath, codexPath: fakeCodex });
  await setup.service.session('run-one', setup.live, { iteration: 3, hat: 'Summary', summary: true, prompt, env: setup.env });
  await setup.service.session('run-one', setup.live, { iteration: 4, hat: 'Hat Designer', designer: true, prompt, env: setup.env });
  assert.equal(setup.calls.creations.length, 2);
  assert.equal(setup.record().backends, undefined);
  assert.equal(setup.calls.activations.length, 0);
});

test('a native backend iteration aborts with the run controller instead of finishing the run', async t => {
  const runPath = await workspace(t);
  process.env.FAKE_CODEX_SCENARIO = 'hang';
  process.env.FAKE_CODEX_TRACE = join(runPath, 'trace.jsonl');
  t.after(() => { delete process.env.FAKE_CODEX_SCENARIO; delete process.env.FAKE_CODEX_TRACE; });
  const setup = fixture({ hats: [{ id: 'reviewer', name: 'Reviewer', backend: 'codex' }], runPath, codexPath: fakeCodex });
  const running = setup.service.session('run-one', setup.live, { iteration: 2, hat: 'Reviewer', hatId: 'reviewer', prompt, env: setup.env });
  setTimeout(() => setup.live.controller.abort(new Error('stopped from DSH')), 150);
  await assert.rejects(running, error => { assert.equal(error.code, 'ABORTED'); return true; });
  assert.equal(setup.record().backends[0].status, 'failed');
  assert.equal(setup.calls.creations.length, 0);
});
