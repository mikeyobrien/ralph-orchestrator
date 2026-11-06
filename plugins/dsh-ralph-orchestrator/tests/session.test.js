import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { RalphOrchestrator } from '../index.js';

// External Host/AgentFactory fixture only: role mounting supplies tools; writing a
// header alone supplies none. Real DSH composition/LLM behavior is verified live.
const roleTools = {
  'ralph-worker': ['bash', 'read', 'write', 'edit', 'glob', 'grep'],
  'ralph-hat-designer': ['read', 'glob', 'grep'],
  'ralph-summary': ['read', 'glob', 'grep'],
};
const schema = name => ({ name, description: `Fixture capability ${name}`, parameters: { type: 'object' } });
const copy = value => structuredClone(value);
const immediate = () => new Promise(resolve => setImmediate(resolve));
function deferred() { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; }

function fixture(controls = {}) {
  const original = { id: 'run-one', mode: 'run', name: 'project-task', taskName: 'task', status: 'running', prompt: 'Original objective', path: '/original/retained-worktree', sourcePath: '/source/must-not-change', workspaceId: 'workspace-one', summaryId: 'reserved-summary', iteration: 0, maxIterations: 4, permissionPreset: 'workspace', model: { provider: 'test', model: 'test' }, sessions: [], drafts: [], createdAt: 1 };
  let record = copy(original);
  const observed = { creations: [], phases: [], followups: [], injections: [], attachments: [], persisted: new Map(), pendingTurns: new Map(), cancellations: [], disposed: [], policies: [], visibleAtFollowup: [], registrations: [], domainClosed: false };
  const listeners = new Set();
  const bound = new WeakMap();
  const ctx = {
    agentPresets: {
      async mount(agentCtx, id) {
        observed.phases.push('mount-start');
        if (controls.mountGate) await controls.mountGate;
        await Promise.resolve();
        if (controls.mountError) throw controls.mountError;
        if (!controls.metadataOnly) {
          assert.ok(roleTools[id], `unexpected role ${id}`);
          bound.set(agentCtx, id);
          for (const name of roleTools[id]) agentCtx.tools.register(schema(name));
        }
        observed.phases.push('mount-complete');
        // This is intentionally NOT a setup commit: returning it from setup
        // would fail the actual AgentFactory's optional commit contract.
        return { id, name: `Preset ${id}` };
      },
      composedPreset: agentCtx => bound.get(agentCtx),
    },
    tools: { schemas: agent => [...agent.ctx.definitions.values()].map(({ name, description, parameters }) => ({ name, description, parameters })) },
    on(name, listener) { assert.equal(name, 'session/event'); listeners.add(listener); return () => listeners.delete(listener); },
    permissionPresets: { catalog: () => ({ options: [], defaultPreset: 'workspace' }), set(session, preset) { observed.policies.push({ id: session.id, preset }); } },
    workspaceRegistry: {
      list: () => [],
      get(id) { assert.equal(id, original.workspaceId); return { async attachSession(sessionId) { observed.attachments.push(sessionId); } }; },
    },
    sessions: { async flush(session) { observed.persisted.set(session.id, { header: copy(session.header), events: copy(session.ownEvents()) }); return true; } },
    sessionPersistence: {
      async stat(id) { const stored = observed.persisted.get(id); return stored ? { header: copy(stored.header), revision: 'fixture-revision' } : undefined; },
      async open(id, access) {
        assert.equal(access, 'read');
        const stored = observed.persisted.get(id);
        assert.ok(stored);
        return { header: copy(stored.header), async read(offset, length) { return { eventState: 'detached', events: copy(stored.events.slice(offset, offset + length)) }; }, async close() {} };
      },
    },
    sessionTitle: { rename(session, title) { session.append('session/title', { title }); return { title }; } },
    subagents: { async drainDescendants() {} }, jobs: { list: () => [] },
    agents: {
      async create(options) {
        observed.creations.push({ sessionId: options.sessionId, meta: copy(options.meta), seed: options.seed, parentAgent: options.parentAgent });
        const log = [];
        const definitions = new Map();
        const agentCtx = { definitions, tools: { register(definition) { definitions.set(definition.name, definition); observed.registrations.push(definition.name); } } };
        let published = false;
        const session = {
          id: options.sessionId, header: { id: options.sessionId, cwd: options.meta.cwd, agentPreset: options.meta.agentPreset, isSeeded: false }, ownEvents: () => log,
          append(type, data, surface) {
            if (type === 'user/message') assert.deepEqual(surface, { surfaceOp: 'append' });
            const event = { type, seq: log.length, data: copy(data), ...(surface || {}) };
            log.push(event);
            for (const listener of listeners) listener(session, event);
            return event;
          },
        };
        const agent = {
          id: session.id, ctx: agentCtx, session,
          cancel(cause) {
            observed.cancellations.push({ id: session.id, cause });
            observed.pendingTurns.delete(session.id);
            controls.onCancel?.({ id: session.id, preset: options.meta.agentPreset, cause });
          },
          async whenIdle() { if (options.meta.agentPreset === 'ralph-worker' && controls.workerIdleGate) await controls.workerIdleGate; },
          inject(message) { assert.equal(published, true); observed.injections.push(copy(message)); },
          followup(message) {
            assert.equal(published, true, 'no model input before factory publication');
            observed.phases.push('followup');
            observed.followups.push(copy(message));
            observed.visibleAtFollowup.push(ctx.tools.schemas(agent).map(tool => tool.name));
            session.append('user/message', message, { surfaceOp: 'append' });
            const finish = (reason = { kind: 'completed' }) => {
              observed.pendingTurns.delete(session.id);
              if (reason.kind === 'completed') session.append('assistant/message', { message: { content: [{ type: 'text', text: 'External agent handoff, not claimed live evidence' }] } });
              session.append('turn/end', { reason });
            };
            if (controls.delayedTurns) observed.pendingTurns.set(session.id, finish);
            else finish();
            controls.onFollowup?.({ id: session.id, preset: options.meta.agentPreset });
          },
        };
        const setupResult = await options.setup?.(agentCtx, agent);
        assert.equal(setupResult, undefined, 'setup must not return AgentPreset metadata instead of void');
        observed.phases.push('published');
        published = true;
        return { agent, async dispose() { observed.disposed.push(session.id); } };
      },
    },
  };
  const service = new RalphOrchestrator(ctx, { sessionTimeoutMs: 1000 });
  service.runs = {
    get: id => id === record.id ? record : undefined,
    entries: () => new Map([[record.id, record]]).entries(),
    async update(id, transform) { assert.equal(id, record.id); record = copy(transform(record)); return record; },
  };
  service.configurations = { entries: () => new Map().entries() };
  service.domain = { async close() { observed.domainClosed = true; } };
  const live = { controller: new AbortController(), handles: new Set() };
  const prompt = 'Exact native Ralph assembled prompt λ🦀\n\n<hat>Keep this byte-for-byte</hat>\r\n';
  return { service, original, observed, live, prompt, record: () => record };
}

for (const role of [
  { name: 'worker', preset: 'ralph-worker', options: {}, expected: ['bash', 'read', 'write', 'edit', 'glob', 'grep', 'ralph', 'ralph_iteration_title'], absent: ['ralph_propose_configuration', 'mcp__playwright-mcp__browser_navigate'] },
  { name: 'designer', preset: 'ralph-hat-designer', options: { designer: true }, expected: ['read', 'glob', 'grep', 'ralph_iteration_title', 'ralph_propose_configuration'], absent: ['bash', 'write', 'edit', 'ralph', 'mcp__playwright-mcp__browser_navigate'] },
  { name: 'summary', preset: 'ralph-summary', options: { summary: true }, expected: ['read', 'glob', 'grep'], absent: ['bash', 'write', 'edit', 'ralph', 'ralph_iteration_title', 'ralph_propose_configuration', 'mcp__playwright-mcp__browser_navigate'] },
]) {
  test(`real session boundary mounts ${role.name} capabilities before delivering the exact native prompt`, async () => {
    const setup = fixture();
    const output = await setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Builder', prompt: setup.prompt, env: { RALPH_BIN: '/exact/ralph' }, ...role.options });
    assert.equal(output, 'External agent handoff, not claimed live evidence');
    assert.deepEqual(setup.observed.phases, ['mount-start', 'mount-complete', 'published', 'followup']);
    assert.equal(setup.observed.creations.length, 1);
    const creation = setup.observed.creations[0];
    assert.equal(creation.meta.cwd, '/original/retained-worktree');
    assert.equal(creation.meta.agentPreset, role.preset);
    assert.equal(creation.seed, undefined); assert.equal(creation.parentAgent, undefined);
    assert.equal(setup.observed.followups.length, 1);
    assert.equal(setup.observed.followups[0].content[0].text, setup.prompt);
    assert.deepEqual(setup.observed.visibleAtFollowup[0].toSorted(), role.expected.toSorted());
    for (const name of role.absent) assert.equal(setup.observed.visibleAtFollowup[0].includes(name), false);
    const ref = setup.record().sessions.find(item => item.id === creation.sessionId);
    assert.equal(ref.agentPreset, role.preset);
    assert.equal(ref.cwd, '/original/retained-worktree'); assert.equal(ref.isSeeded, false);
    assert.deepEqual(ref.toolNames.toSorted(), role.expected.toSorted());
    assert.ok(ref.toolSchemaBytes > 0);
    assert.equal(ref.status, 'completed');
    assert.equal(setup.observed.persisted.get(creation.sessionId).header.isSeeded, false);
    assert.equal(setup.record().sourcePath, '/source/must-not-change');
    assert.deepEqual(setup.observed.attachments, [creation.sessionId]);
    assert.deepEqual(setup.observed.disposed, [creation.sessionId]);
    assert.equal(setup.live.handles.size, 0);
    if (role.name === 'summary') assert.equal(creation.sessionId, 'reserved-summary');
  });
}

test('asynchronous preset activation must settle before factory publication or any prompt', async () => {
  let release;
  const mountGate = new Promise(resolve => { release = resolve; });
  const setup = fixture({ mountGate });
  const running = setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Builder', prompt: setup.prompt });
  // The durable session reservation precedes AgentFactory creation.
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(setup.observed.phases, ['mount-start']);
  assert.deepEqual(setup.observed.followups, []);
  assert.equal(setup.observed.persisted.size, 0);
  release(); await running;
  assert.deepEqual(setup.observed.phases, ['mount-start', 'mount-complete', 'published', 'followup']);
});

test('a preset name in metadata without an actual bound composition is rejected before model input', async () => {
  const setup = fixture({ metadataOnly: true });
  await assert.rejects(setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Builder', prompt: setup.prompt }), /did not mount its required role preset/);
  assert.equal(setup.observed.creations[0].meta.agentPreset, 'ralph-worker');
  assert.deepEqual(setup.observed.followups, []); assert.deepEqual(setup.observed.injections, []);
  assert.equal(setup.record().sessions[0].status, 'failed');
  assert.equal(setup.observed.disposed.length, 1);
});

test('preset activation failures remain visible and never deliver a prompt', async () => {
  const error = new Error('Preset activation failed at external boundary');
  const setup = fixture({ mountError: error });
  await assert.rejects(setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Builder', prompt: setup.prompt }), error);
  assert.deepEqual(setup.observed.followups, []); assert.deepEqual(setup.observed.injections, []);
  assert.equal(setup.observed.persisted.size, 0);
});

test('Stop acknowledges genuine work quiescence without waiting for the owned summary model', { timeout: 3000 }, async t => {
  const workerStarted = deferred(), workerCancelled = deferred(), workerQuiescent = deferred(), summaryStarted = deferred();
  const controls = {
    delayedTurns: true,
    workerIdleGate: workerQuiescent.promise,
    onFollowup({ preset }) { (preset === 'ralph-worker' ? workerStarted : summaryStarted).resolve(); },
    onCancel({ preset }) { if (preset === 'ralph-worker') workerCancelled.resolve(); },
  };
  const setup = fixture(controls);
  setup.service.live.set('run-one', setup.live);
  setup.live.task = setup.service.session('run-one', setup.live, { iteration: 1, hat: 'Builder', prompt: setup.prompt }).catch(error => {
    assert.equal(error.code, 'ABORTED');
  });
  let stopping;
  t.after(async () => {
    controls.delayedTurns = false;
    workerQuiescent.resolve();
    for (const finish of setup.observed.pendingTurns.values()) finish();
    await Promise.allSettled([setup.live.task, stopping, setup.service.live.get('run-one')?.task]);
  });
  await workerStarted.promise;
  let acknowledged = false;
  stopping = setup.service.stop('run-one').then(result => { acknowledged = true; return result; });
  await workerCancelled.promise;
  await immediate();
  assert.equal(acknowledged, false, 'Stop must still wait for actual worker quiescence');
  assert.equal(setup.record().status, 'stopping');
  assert.equal(setup.observed.creations.length, 1, 'no summary while work is still settling');
  workerQuiescent.resolve();
  await summaryStarted.promise;
  const result = await Promise.race([stopping, immediate().then(() => null)]);
  assert.equal(result?.status, 'stopped', 'Stop must return while summary turn/end is still pending');
  const summaryLive = setup.service.live.get('run-one');
  assert.ok(summaryLive && summaryLive !== setup.live);
  assert.ok(summaryLive.task instanceof Promise, 'background finalization remains lifecycle-owned');
  assert.equal(setup.observed.pendingTurns.has('reserved-summary'), true);
  assert.equal(setup.record().sessions.find(item => item.id === 'reserved-summary').status, 'running');
  assert.deepEqual(setup.observed.creations.map(item => item.meta.agentPreset), ['ralph-worker', 'ralph-summary']);
  assert.equal(setup.observed.disposed.includes(setup.observed.creations[0].sessionId), true);

  setup.observed.pendingTurns.get('reserved-summary')();
  await summaryLive.task;
  assert.equal(setup.record().status, 'stopped');
  assert.equal(setup.record().sessions.find(item => item.id === 'reserved-summary').status, 'completed');
  assert.equal(setup.observed.persisted.has('reserved-summary'), true);
  assert.equal(setup.record().sessions.filter(item => item.id === 'reserved-summary').length, 1);
  assert.equal(setup.service.live.has('run-one'), false);
  await setup.service.stop('run-one');
  assert.equal(setup.observed.creations.length, 2, 'repeated Stop must not start more work or a second summary');
});

test('background summary failure is visible and late cleanup cannot remove a replacement owner', { timeout: 3000 }, async t => {
  const summaryStarted = deferred();
  const controls = { delayedTurns: true, onFollowup() { summaryStarted.resolve(); } };
  const setup = fixture(controls);
  const stopping = setup.service.stop('run-one');
  t.after(async () => {
    controls.delayedTurns = false;
    for (const finish of setup.observed.pendingTurns.values()) finish();
    await Promise.allSettled([stopping]);
  });
  assert.equal((await stopping).status, 'stopped');
  await summaryStarted.promise;
  const summaryLive = setup.service.live.get('run-one');
  const replacement = { controller: new AbortController(), handles: new Set() };
  setup.service.live.set('run-one', replacement);
  setup.observed.pendingTurns.get('reserved-summary')({ kind: 'error', error: { message: 'External summary provider failed' } });
  await summaryLive.task;
  assert.equal(setup.record().status, 'stopped');
  assert.match(setup.record().summaryError, /External summary provider failed/);
  assert.equal(setup.record().sessions.find(item => item.id === 'reserved-summary').status, 'failed');
  assert.equal(setup.service.live.get('run-one'), replacement);
  assert.deepEqual(setup.observed.creations.map(item => item.sessionId), ['reserved-summary']);
  assert.deepEqual(setup.observed.disposed, ['reserved-summary']);
});

test('shutdown cancels and joins a pending owned stop-summary without waiting for model completion', { timeout: 3000 }, async t => {
  const summaryStarted = deferred();
  const controls = { delayedTurns: true, onFollowup() { summaryStarted.resolve(); } };
  const setup = fixture(controls);
  setup.live.task = Promise.resolve();
  setup.service.live.set('run-one', setup.live);
  setup.service.ipcDirectory = await mkdtemp(join(tmpdir(), 'dsh-stop-summary-close-'));
  setup.service.server = { close(done) { done(); } };
  t.after(async () => {
    controls.delayedTurns = false;
    for (const finish of setup.observed.pendingTurns.values()) finish();
    await rm(setup.service.ipcDirectory, { recursive: true, force: true });
  });
  const result = await setup.service.dispatch({ action: 'stop', runId: 'run-one' }, { user: true });
  assert.equal(result.status, 'stopped');
  await summaryStarted.promise;
  const summaryLive = setup.service.live.get('run-one');
  assert.ok(summaryLive.task instanceof Promise);
  await setup.service.close();
  assert.equal(summaryLive.controller.signal.aborted, true);
  assert.equal(setup.observed.pendingTurns.size, 0);
  assert.equal(setup.observed.domainClosed, true);
  assert.deepEqual(setup.observed.disposed, ['reserved-summary']);
  assert.equal(setup.service.live.size, 0);
  assert.equal(setup.record().status, 'stopped');
  assert.match(setup.record().summaryError, /cancelled/);
  assert.equal(setup.observed.creations.length, 1);
});
