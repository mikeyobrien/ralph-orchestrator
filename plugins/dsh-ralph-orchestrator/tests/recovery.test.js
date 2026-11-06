import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { RalphOrchestrator } from '../index.js';

const copy = value => structuredClone(value);
function table(values = []) {
  const rows = new Map(values.map(value => [value.id, copy(value)]));
  return {
    get: id => rows.get(id), entries: () => rows.entries(),
    async put(id, value) { rows.set(id, copy(value)); },
    async update(id, transform) { const value = copy(transform(rows.get(id))); rows.set(id, value); return value; },
  };
}
function run(patch = {}) {
  return { id: 'run-one', requestId: 'request-one', name: 'project-task', taskName: 'task', mode: 'run', prompt: 'Original objective, not a claim of success', status: 'running', path: '/original/worktree', workspaceId: 'run-workspace', sourcePath: '/source/unchanged', summaryId: 'reserved-summary', iteration: 1, maxIterations: 3, permissionPreset: 'workspace', model: { provider: 'test', model: 'test' }, createdAt: 1, updatedAt: 1, drafts: [], sessions: [{ id: 'iteration-one', title: '01 / 03 - Builder', status: 'failed', handoff: 'Build command failed; no passing tests observed.', error: 'Build failure' }], ...patch };
}
function fixture({ runs = table([run()]), artifacts = new Map(), controls = {} } = {}) {
  const observed = { creates: [], mounts: [], wakes: [], attached: [], titles: [], disposed: [], readersClosed: [], policy: [], writes: [], domainClosed: false };
  const events = new Set();
  const compositions = new WeakMap();
  const ctx = {
    agentPresets: {
      async mount(agentCtx, id) {
        await Promise.resolve();
        if (controls.mountError) throw controls.mountError;
        compositions.set(agentCtx, id);
        observed.mounts.push(id);
        return { id, name: 'Mounted preset metadata, not an AgentSetupCommit' };
      },
      composedPreset: agentCtx => compositions.get(agentCtx),
    },
    tools: { schemas: agent => [...agent.ctx.definitions.values()].map(({ name, description, parameters }) => ({ name, description, parameters })) },
    permissionPresets: { catalog: () => ({ options: [], defaultPreset: 'workspace' }), set: (session, preset) => observed.policy.push({ id: session.id, preset }) },
    workspaceRegistry: {
      list: () => [{ id: 'run-workspace', path: '/original/worktree', title: 'project-task' }],
      get: id => id === 'run-workspace' ? { async attachSession(sessionId) { observed.attached.push(sessionId); } } : undefined,
    },
    sessionPersistence: {
      async stat(id) { if (controls.statError) throw controls.statError; return artifacts.has(id) ? { header: copy(artifacts.get(id).header), revision: 'revision' } : undefined; },
      async open(id, access) {
        assert.equal(access, 'read');
        if (controls.openError) throw controls.openError;
        if (!artifacts.has(id)) throw new Error('Missing artifact');
        return { header: copy(artifacts.get(id).header), async read(offset, length) { if (controls.readError) throw controls.readError; return { eventState: 'detached', events: copy(artifacts.get(id).events.slice(offset, offset + length)) }; }, async close() { observed.readersClosed.push(id); } };
      },
    },
    on(name, listener) { assert.equal(name, 'session/event'); events.add(listener); return () => events.delete(listener); },
    sessions: {
      async flush(session) {
        if (controls.flushGate) await controls.flushGate(session);
        if (controls.flushError) throw controls.flushError;
        artifacts.set(session.id, { header: copy(session.header), events: copy(session.ownEvents()) });
        return controls.flushParticipated !== false;
      },
    },
    sessionTitle: { rename(session, title) { observed.titles.push(title); session.append('session/title', { title }); return { title }; } },
    agents: {
      async create(options) {
        observed.creates.push(copy({ ...options, setup: undefined }));
        if (controls.createError) throw controls.createError;
        if (artifacts.has(options.sessionId)) throw new Error('Duplicate persistent identity');
        const log = [];
        const session = {
          id: options.sessionId, header: { id: options.sessionId, isSeeded: false, ...options.meta }, ownEvents: () => log,
          append(type, data, surface) {
            if (type === 'user/message') {
              assert.deepEqual(surface, { surfaceOp: 'append' });
              assert.equal(data.role, 'user'); assert.equal(typeof data.id, 'string');
            }
            const event = { type, seq: log.length, data: copy(data), ...(surface || {}) };
            log.push(event); observed.writes.push({ id: session.id, event: copy(event) });
            for (const listener of events) listener(session, event);
            return event;
          },
        };
        const definitions = new Map();
        const agentCtx = { definitions, tools: { register(definition) { definitions.set(definition.name, definition); } } };
        const agent = {
          id: session.id, ctx: agentCtx, session, cancel() {}, async whenIdle() {},
          inject(message) { observed.wakes.push({ kind: 'inject', message }); },
          followup(message) {
            observed.wakes.push({ kind: 'followup', message });
            session.append('user/message', message, { surfaceOp: 'append' });
            session.append('assistant/message', { message: { content: [{ type: 'text', text: 'Recorded model summary' }] } });
            session.append('turn/end', { reason: { kind: 'completed' } });
          },
        };
        const setupResult = await options.setup?.(agentCtx, agent);
        assert.equal(setupResult, undefined, 'orchestrator setup must not return preset metadata as an AgentSetupCommit');
        return { agent, async dispose() { observed.disposed.push(session.id); if (controls.disposeError) throw controls.disposeError; } };
      },
    },
    subagents: { async drainDescendants() {} }, jobs: { list: () => [] },
  };
  const service = new RalphOrchestrator(ctx, { sessionTimeoutMs: 1000 });
  service.runs = runs;
  service.configurations = table([{ id: 'config-one', name: 'Workflow', revision: 1, yaml: 'private full YAML', hats: [{ id: 'builder', name: 'Builder', instructions: 'private long instructions' }] }]);
  service.domain = { async close() { observed.domainClosed = true; assert.equal(observed.disposed.length, observed.creates.filter(() => !controls.createError).length); } };
  return { service, ctx, observed, artifacts, controls, runs };
}

test('restart and concurrent reads recover one reserved summary at original cwd without waking or replaying work', async () => {
  const setup = fixture();
  await Promise.all([setup.service.reconcile(), setup.service.dispatch({ action: 'state' }), setup.service.reconcile(), setup.service.dispatch({ action: 'state', runId: 'run-one' })]);
  assert.equal(setup.observed.creates.length, 1);
  assert.equal(setup.artifacts.size, 1);
  const summary = setup.artifacts.get('reserved-summary');
  assert.deepEqual(summary.header, { id: 'reserved-summary', cwd: '/original/worktree', agentPreset: 'ralph-summary', isSeeded: false });
  assert.equal(setup.observed.creates[0].parentAgent, undefined);
  assert.equal(setup.observed.creates[0].seed, undefined);
  assert.deepEqual(setup.observed.wakes, []);
  assert.deepEqual(setup.observed.mounts, ['ralph-summary']);
  assert.deepEqual(setup.observed.attached, ['reserved-summary']);
  assert.deepEqual(setup.observed.titles, ['DONE - Summary']);
  assert.deepEqual(setup.observed.disposed, ['reserved-summary']);
  const text = summary.events.find(event => event.type === 'user/message').data.content[0].text;
  assert.match(text, /Outcome: interrupted/);
  assert.match(text, /not that its objective succeeded/);
  assert.match(text, /no iteration was replayed/i);
  assert.match(text, /iteration-one/); assert.match(text, /Build command failed/);
  assert.match(text, /DSH restarted/);
  assert.equal(setup.runs.get('run-one').status, 'interrupted');
  assert.equal(setup.runs.get('run-one').sourcePath, '/source/unchanged');
  assert.equal(setup.runs.get('run-one').sessions.filter(item => item.id === 'reserved-summary').length, 1);

  const restarted = fixture({ runs: setup.runs, artifacts: setup.artifacts });
  await restarted.service.dispatch({ action: 'state' });
  assert.deepEqual(restarted.observed.creates, []);
  assert.deepEqual(restarted.observed.writes, []);
  assert.deepEqual(restarted.observed.wakes, []);
});

test('failed database-only reservation is not mistaken for a real summary and duplicate references collapse', async () => {
  const original = run({ status: 'failed', error: 'Native Ralph disconnected' });
  original.sessions.push({ id: original.summaryId, title: 'DONE - Summary', status: 'failed', error: 'Preset not mounted' }, { id: original.summaryId, status: 'running' });
  const setup = fixture({ runs: table([original]) });
  const { run: result } = await setup.service.dispatch({ action: 'state', runId: original.id });
  assert.equal(result.sessions.filter(item => item.id === original.summaryId).length, 1);
  assert.equal(result.error, 'Native Ralph disconnected');
  assert.equal(result.status, 'failed');
  assert.equal(result.sessions.at(-1).kind, 'recorded-evidence');
  assert.match(setup.artifacts.get(original.summaryId).events.at(-1).data.content[0].text, /Native Ralph disconnected/);
  assert.deepEqual(setup.observed.wakes, []);
});

test('normal finalization also checks real persistence instead of trusting a failed database reservation', async () => {
  const original = run({ status: 'failed', sessions: [{ id: 'reserved-summary', title: 'DONE - Summary', status: 'failed' }] });
  const setup = fixture({ runs: table([original]) });
  const live = { controller: new AbortController(), handles: new Set() };
  await setup.service.finalize(original.id, live);
  assert.equal(setup.observed.creates.length, 1);
  assert.equal(setup.artifacts.size, 1);
  assert.equal(setup.observed.wakes.filter(item => item.kind === 'followup').length, 1);
  assert.equal(setup.runs.get(original.id).sessions.filter(item => item.id === original.summaryId).length, 1);
  assert.equal(setup.runs.get(original.id).sessions[0].status, 'completed');
});

test('existing readable persisted summary remains byte-for-byte untouched even with missing database reference', async () => {
  const artifact = { header: { id: 'reserved-summary', cwd: '/original/worktree', isSeeded: false }, events: [{ type: 'session/title', seq: 0, data: { title: 'DONE - Summary' } }, { type: 'user/message', seq: 1, data: { content: [{ type: 'text', text: 'Original evidence, do not replace' }] } }] };
  const setup = fixture({ runs: table([run({ status: 'completed' })]), artifacts: new Map([['reserved-summary', copy(artifact)]]) });
  await setup.service.reconcile();
  assert.deepEqual(setup.artifacts.get('reserved-summary'), artifact);
  assert.deepEqual(setup.observed.creates, []); assert.deepEqual(setup.observed.titles, []);
  assert.deepEqual(setup.observed.readersClosed, ['reserved-summary']);
  assert.equal(setup.runs.get('run-one').sessions.at(-1).id, 'reserved-summary');
});

test('corrupt or unsupported persistence is a visible failure, never absence or permission to recreate', async t => {
  for (const boundary of ['statError', 'openError', 'readError']) await t.test(boundary, async () => {
    const error = Object.assign(new Error('Stored summary cannot be interpreted'), { code: 'SESSION_FORMAT_UNSUPPORTED' });
    const setup = fixture({ runs: table([run({ status: 'completed' })]), artifacts: new Map([['reserved-summary', { header: {}, events: [] }]]), controls: { [boundary]: error } });
    await assert.rejects(setup.service.dispatch({ action: 'state' }), error);
    assert.deepEqual(setup.observed.creates, []);
    assert.equal(setup.runs.get('run-one').summaryError, error.message);
    if (boundary === 'readError') assert.deepEqual(setup.observed.readersClosed, ['reserved-summary']);
  });
});

test('creation, flush and disposal failures surface and do not allocate a replacement summary ID', async t => {
  for (const boundary of ['createError', 'flushError', 'disposeError']) await t.test(boundary, async () => {
    const error = new Error(`${boundary} boundary failed`);
    const setup = fixture({ controls: { [boundary]: error } });
    await assert.rejects(setup.service.reconcile(), error);
    assert.equal(setup.runs.get('run-one').summaryId, 'reserved-summary');
    assert.equal(setup.runs.get('run-one').summaryError, error.message);
    if (boundary !== 'createError') assert.deepEqual(setup.observed.disposed, ['reserved-summary']);
    delete setup.controls[boundary];
    await setup.service.reconcile();
    assert.equal(setup.artifacts.size, 1);
    assert.ok(setup.observed.creates.every(item => item.sessionId === 'reserved-summary'));
    assert.deepEqual(setup.observed.wakes, []);
  });
});

test('active owned execution, review drafts and runs without worktrees are not replayed or finalized', async () => {
  const setup = fixture({ runs: table([run(), run({ id: 'review', status: 'review' }), run({ id: 'no-worktree', status: 'failed', path: undefined })]) });
  setup.service.live.set('run-one', { process: {} });
  await setup.service.dispatch({ action: 'state' });
  assert.deepEqual(setup.observed.creates, []);
  assert.equal(setup.runs.get('run-one').status, 'running');
  assert.equal(setup.runs.get('review').status, 'review');
});

test('tool state is recent metadata with explicit details while user/UI state retains complete records', async () => {
  const originals = Array.from({ length: 25 }, (_, index) => run({ id: `run-${index}`, status: 'completed', path: undefined, createdAt: index, prompt: `Full original prompt ${index}`, yaml: 'Full YAML', sessions: [{ id: `session-${index}`, handoff: 'Full recorded handoff' }] }));
  const setup = fixture({ runs: table(originals) });
  const state = await setup.service.dispatch({ action: 'state' });
  assert.equal(state.totalRuns, 25); assert.equal(state.runs.length, 20); assert.equal(state.runs[0].id, 'run-24');
  assert.equal(state.runs[0].sessionCount, 1);
  assert.equal(JSON.stringify(state).includes('Full original prompt'), false);
  assert.equal(JSON.stringify(state).includes('Full recorded handoff'), false);
  assert.equal(JSON.stringify(state).includes('Full YAML'), false);
  assert.equal(JSON.stringify(state).includes('private long instructions'), false);
  assert.deepEqual(JSON.parse(JSON.stringify(state)), state, 'tool metadata is lossless JSON even before worktree allocation');
  assert.equal(state.configurations[0].id, 'config-one');
  const detail = await setup.service.dispatch({ action: 'state', runId: 'run-0' });
  assert.equal(detail.run.prompt, 'Full original prompt 0'); assert.equal(detail.run.sessions[0].handoff, 'Full recorded handoff');
  const ui = await setup.service.dispatch({ action: 'state' }, { user: true });
  assert.equal(ui.runs.length, 25); assert.equal(ui.configurations[0].yaml, 'private full YAML');
});

test('agent calls cannot accept configurations after recovery', async () => {
  const setup = fixture({ runs: table([run({ status: 'review', mode: 'design', draft: { revision: 1, yaml: 'draft' } })]) });
  await assert.rejects(setup.service.dispatch({ action: 'accept', runId: 'run-one', revision: 1, name: 'Unauthorized' }), error => error.code === 'USER_ONLY');
  assert.equal(setup.runs.get('run-one').status, 'review');
  assert.deepEqual([...setup.service.configurations.entries()].map(([id]) => id), ['config-one']);
});

test('shutdown waits for in-flight recovery disposal before closing its domain', async t => {
  let release, began;
  const blocked = new Promise(resolve => { release = resolve; });
  const started = new Promise(resolve => { began = resolve; });
  const setup = fixture({ controls: { async flushGate() { began(); await blocked; } } });
  setup.service.ipcDirectory = await mkdtemp(join(tmpdir(), 'dsh-recovery-close-'));
  setup.service.server = { close(done) { done(); } };
  t.after(() => rm(setup.service.ipcDirectory, { recursive: true, force: true }));
  const recovering = setup.service.reconcile();
  await started;
  const closing = setup.service.close();
  assert.equal(setup.observed.domainClosed, false);
  const denied = assert.rejects(setup.service.reconcile(), /shutting down/);
  release();
  await Promise.all([recovering, closing, denied]);
  assert.equal(setup.observed.domainClosed, true); assert.deepEqual(setup.observed.disposed, ['reserved-summary']);
});

test('bridge rejects concurrent authorized invocations before asynchronous state validation and releases on failure', async () => {
  const setup = fixture({ runs: table([run({ path: process.cwd(), status: 'running', iteration: 0 })]) });
  let release, began;
  const blocked = new Promise(resolve => { release = resolve; });
  const started = new Promise(resolve => { began = resolve; });
  let stateRequests = 0;
  const live = { controller: new AbortController(), handles: new Set(), invocations: new Set(), token: 'same-secret', process: { async request() { stateRequests++; began(); await blocked; throw new Error('Engine disconnected'); } } };
  setup.service.live.set('run-one', live);
  const request = { runId: 'run-one', token: 'same-secret', id: 'first', prompt: 'Assembled prompt', cwd: process.cwd(), env: { RALPH_WORKSPACE_ROOT: process.cwd(), RALPH_EVENTS_FILE: join(process.cwd(), 'events.jsonl') } };
  const first = assert.rejects(setup.service.bridge(request, new AbortController().signal), /Engine disconnected/);
  await started;
  await assert.rejects(setup.service.bridge({ ...request, id: 'second' }, new AbortController().signal), /Overlapping/);
  release(); await first;
  assert.equal(stateRequests, 1);
  await assert.rejects(setup.service.bridge({ ...request, id: 'third' }, new AbortController().signal), /Engine disconnected/);
  assert.equal(stateRequests, 2);
  assert.deepEqual(setup.observed.creates, []);
});
