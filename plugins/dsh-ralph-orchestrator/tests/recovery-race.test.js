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
  return { id: 'design-one', requestId: 'request-design-one', name: 'design-task', taskName: 'task', mode: 'design', prompt: 'Original objective, not a claim of success', status: 'review', path: '/design/worktree', workspaceId: 'design-workspace', sourcePath: '/source/unchanged', summaryId: 'reserved-summary', iteration: 0, maxIterations: 3, permissionPreset: 'workspace', model: { provider: 'test', model: 'test' }, createdAt: 1, updatedAt: 1, drafts: [], sessions: [], ...patch };
}
const BUILD = 'hats:\n  builder:\n    name: Builder\n    triggers: []\n    publishes: []\n    instructions: Build';
function fixture({ runs = table([run()]), controls = {} } = {}) {
  const artifacts = new Map();
  const observed = { creates: [], mounts: [], wakes: [], attached: [], titles: [], disposed: [], policy: [], writes: [], domainClosed: false, validates: 0 };
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
      list: () => [{ id: 'design-workspace', path: '/design/worktree', title: 'design-task' }],
      get: id => id === 'design-workspace' ? { async attachSession(sessionId) { observed.attached.push(sessionId); } } : undefined,
    },
    on(name, listener) { if (name === 'session/event') events.add(listener); return () => events.delete(listener); },
    sessions: { async flush(session) { artifacts.set(session.id, { header: copy(session.header), events: copy(session.ownEvents()) }); return true; } },
    sessionPersistence: {
      async stat(id) { return artifacts.has(id) ? { header: copy(artifacts.get(id).header), revision: 'revision' } : undefined; },
      async open(id, access) {
        assert.equal(access, 'read');
        if (!artifacts.has(id)) throw new Error('Missing artifact');
        return { header: copy(artifacts.get(id).header), async read(offset, length) { return { eventState: 'detached', events: copy(artifacts.get(id).events.slice(offset, offset + length)) }; }, async close() {} };
      },
    },
    sessionTitle: { rename(session, title) { observed.titles.push(title); session.append('session/title', { title }); return { title }; } },
    agents: {
      async create(options) {
        observed.creates.push(copy({ ...options, setup: undefined }));
        if (controls.createError) throw controls.createError;
        const log = [];
        const session = {
          id: options.sessionId, header: { id: options.sessionId, isSeeded: false, ...options.meta }, ownEvents: () => log,
          append(type, data, surface) {
            if (type === 'user/message') {
              assert.deepEqual(surface, { surfaceOp: 'append' });
              assert.equal(data.role, 'user'); assert.equal(typeof data.id, 'string');
            }
            const event = { type, seq: log.length, data: copy(data), ...(surface || {}) };
            log.push(event); observed.wakes.push({ kind: 'followup' });
            for (const listener of events) listener(session, event);
            return event;
          },
        };
        const definitions = new Map();
        const agentCtx = { definitions, tools: { register(definition) { definitions.set(definition.name, definition); } } };
        const isDesigner = options.meta?.agentPreset === 'ralph-hat-designer';
        const agent = {
          id: session.id, ctx: agentCtx, session, cancel() {}, async whenIdle() {},
          inject(message) { observed.wakes.push({ kind: 'inject', message }); },
          followup(message) {
            observed.wakes.push({ kind: 'followup', message });
            session.append('user/message', message, { surfaceOp: 'append' });
            if (isDesigner) agent.followupDesign();
          },
          async followupDesign() {
            if (controls.proposalGate) await controls.proposalGate;
            if (controls.noProposal) {
              session.append('assistant/message', { message: { content: [{ type: 'text', text: 'No draft submitted' }] } });
              session.append('turn/end', { reason: { kind: 'completed' } });
              return;
            }
            if (controls.proposalError) {
              session.append('assistant/message', { message: { content: [{ type: 'text', text: 'Designer failed' }] } });
              session.append('turn/end', { reason: { kind: 'failed', error: { message: controls.proposalError.message } } });
              return;
            }
            const definition = definitions.get('ralph_propose_configuration');
            assert.ok(definition, 'designer tool must be registered');
            try {
              await definition.execute({ yaml: controls.proposalYaml || BUILD });
            } catch (error) {
              observed.proposeError = error;
              session.append('assistant/message', { message: { content: [{ type: 'text', text: 'Propose failed' }] } });
              session.append('turn/end', { reason: { kind: 'failed', error: { message: error.message } } });
              return;
            }
            session.append('assistant/message', { message: { content: [{ type: 'text', text: 'Draft submitted' }] } });
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
  service.domain = { async close() { observed.domainClosed = true; } };
  // Stub Ralph validation; delegated validation runs against this path.
  service.validate = async (yaml, maxIterations, cwd) => {
    observed.validates++;
    if (controls.validateGate) await controls.validateGate;
    if (controls.validateError) throw new Error(controls.validateError.message);
    return { yaml, hats: [{ id: 'builder', name: 'Builder', instructions: 'Build' }], prepared: yaml };
  };
  const settle = async () => { for (let i = 0; i < 5; i++) await new Promise(resolve => setImmediate(resolve)); };
  return { service, ctx, observed, runs, controls, artifacts, settle };
}

test('interrupted design with no draft is restored from the exact supplied YAML', async () => {
  const setup = fixture({ runs: table([run({ status: 'interrupted', error: 'DSH restarted during this run. Work was preserved; execution was not replayed.' })]) });
  const yaml = '# supplied by the user\n' + BUILD + '\n';
  const result = await setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml });
  assert.equal(result.status, 'review');
  assert.equal(result.draft.revision, 1);
  assert.equal(result.draft.source, 'user-supplied');
  assert.equal(result.draft.yaml, yaml, 'restore must preserve the exact input without regeneration');
  assert.equal(result.drafts.length, 1);
  assert.equal(setup.observed.validates, 1);
});

test('a second concurrent restore is refused instead of duplicating the draft', async () => {
  const setup = fixture({ runs: table([run({ status: 'interrupted' })]) });
  const results = await Promise.allSettled([
    setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml: BUILD }),
    setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml: BUILD + ' v2' }),
  ]);
  assert.equal(results.filter(result => result.status === 'fulfilled').length, 1);
  assert.equal(results.filter(result => result.status === 'rejected').length, 1);
  const stored = setup.runs.get('design-one');
  assert.equal(stored.status, 'review');
  assert.equal(stored.drafts.filter(draft => draft.source === 'user-supplied').length, 1);
  assert.equal(stored.draft.revision, 1);
});

test('a restore that is still validating refuses a second restore as busy', async () => {
  let release;
  const validateGate = new Promise(resolve => { release = resolve; });
  const setup = fixture({ runs: table([run({ status: 'interrupted' })]), controls: { validateGate } });
  const first = setup.service.restore({ runId: 'design-one', yaml: BUILD });
  await setup.settle();
  await assert.rejects(setup.service.restore({ runId: 'design-one', yaml: BUILD + ' v2' }), { code: 'BUSY' });
  release();
  const restored = await first;
  assert.equal(restored.draft.revision, 1);
  assert.equal(setup.runs.get('design-one').drafts.length, 1);
});

test('invalid YAML restores the prior interrupted status and leaves draft history untouched', async () => {
  const setup = fixture({ runs: table([run({ status: 'interrupted', error: 'Interrupted' })]), controls: { validateError: new Error('Configuration must be a YAML mapping') } });
  await assert.rejects(setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml: 'not: a: mapping' }), /YAML mapping/);
  const stored = setup.runs.get('design-one');
  assert.equal(stored.status, 'interrupted');
  assert.match(stored.error, /Restore validation failed: Configuration must be a YAML mapping/);
  assert.deepEqual(stored.drafts, []);
  assert.equal(stored.draft, undefined);
});

test('invalid YAML while a draft-less design is in review returns it to review, not a guessed status', async () => {
  const setup = fixture({ runs: table([run({ status: 'review' })]), controls: { validateError: new Error('missing hat name') } });
  await assert.rejects(setup.service.restore({ runId: 'design-one', yaml: 'bad' }), /missing hat name/);
  assert.equal(setup.runs.get('design-one').status, 'review');
});

test('invalid YAML on a failed design returns it to failed', async () => {
  const setup = fixture({ runs: table([run({ status: 'failed', error: 'Designer crashed' })]), controls: { validateError: new Error('invalid trigger') } });
  await assert.rejects(setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml: 'bad' }), /invalid trigger/);
  assert.equal(setup.runs.get('design-one').status, 'failed');
});

test('a draft written while restore validation is in flight is not overwritten', async () => {
  let release;
  const validateGate = new Promise(resolve => { release = resolve; });
  const setup = fixture({ runs: table([run({ status: 'interrupted' })]), controls: { validateGate } });
  const restoring = setup.service.restore({ runId: 'design-one', yaml: BUILD });
  await setup.settle();
  // A concurrent designer commits a newer draft before validation resolves.
  await setup.runs.update('design-one', stored => ({
    ...stored, status: 'review',
    draft: { revision: 1, yaml: 'newer designer draft', hats: [] },
    drafts: [...stored.drafts, { revision: 1, yaml: 'newer designer draft', hats: [] }],
  }));
  release();
  const result = await restoring;
  const stored = setup.runs.get('design-one');
  assert.equal(result.draft.yaml, 'newer designer draft', 'restore must recheck before committing');
  assert.equal(stored.status, 'review');
  assert.equal(stored.drafts.length, 1);
  assert.equal(stored.draft.revision, 1);
});

test('completed and stopped designs cannot be restored or resurrected', async () => {
  for (const status of ['completed', 'stopped']) {
    const setup = fixture({ runs: table([run({ status })]) });
    await assert.rejects(setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml: BUILD }), /interrupted, failed, or draft-less/);
    assert.equal(setup.runs.get('design-one').status, status);
    assert.deepEqual(setup.runs.get('design-one').drafts, []);
  }
});

test('a review with a recorded draft is revised, not overwritten by restore', async () => {
  const setup = fixture({ runs: table([run({ status: 'review', draft: { revision: 2, yaml: 'reviewed draft', hats: [] }, drafts: [{ revision: 1, yaml: BUILD }, { revision: 2, yaml: 'reviewed draft', hats: [] }] })]) });
  await assert.rejects(setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml: BUILD + ' v3' }), /draft-less/);
  const stored = setup.runs.get('design-one');
  assert.equal(stored.draft.revision, 2);
  assert.equal(stored.draft.yaml, 'reviewed draft');
  assert.equal(stored.drafts.length, 2);
});

test('restoring is not accepting: the draft stays in review and no configuration is saved', async () => {
  const setup = fixture({ runs: table([run({ status: 'interrupted' })]) });
  const restored = await setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml: BUILD });
  assert.equal(restored.status, 'review');
  assert.deepEqual([...setup.service.configurations.entries()].map(([id]) => id), ['config-one']);
});

test('a fresh design that is still preparing records its first designer draft', async () => {
  const setup = fixture({ runs: table([run({ status: 'preparing', drafts: [] })]) });
  const live = { controller: new AbortController(), handles: new Set(), token: 'token', invocations: new Set() };
  setup.controls.proposalYaml = BUILD;
  await setup.service.generate('design-one', live);
  const stored = setup.runs.get('design-one');
  assert.equal(stored.status, 'review');
  assert.equal(stored.draft.revision, 1);
  assert.equal(stored.draft.yaml, BUILD);
  assert.equal(stored.drafts.length, 1);
});

test('revise records a new revision and preserves the previous draft', async () => {
  const previous = { revision: 1, yaml: 'draft one', hats: [] };
  const setup = fixture({ runs: table([run({ status: 'review', draft: previous, drafts: [previous] })]) });
  setup.controls.proposalYaml = 'hats:\n  builder:\n    name: Builder v2\n    triggers: []\n    publishes: []\n    instructions: Build';
  await setup.service.dispatch({ action: 'revise', runId: 'design-one', feedback: 'Add a review gate' });
  await setup.service.live.get('design-one').task;
  const stored = setup.runs.get('design-one');
  assert.equal(stored.status, 'review');
  assert.equal(stored.draft.revision, 2);
  assert.equal(stored.draft.feedback, 'Add a review gate');
  assert.equal(stored.drafts.length, 2);
  assert.equal(stored.drafts[0].yaml, 'draft one');
});

test('a failed revision keeps the recorded draft and returns to review', async () => {
  const previous = { revision: 1, yaml: 'draft one', hats: [] };
  const setup = fixture({ runs: table([run({ status: 'review', draft: previous, drafts: [previous] })]) });
  setup.controls.proposalError = new Error('designer crashed');
  await setup.service.dispatch({ action: 'revise', runId: 'design-one', feedback: 'Try again' });
  await setup.service.live.get('design-one').task;
  const stored = setup.runs.get('design-one');
  assert.equal(stored.status, 'review');
  assert.equal(stored.draft.revision, 1);
  assert.equal(stored.draft.yaml, 'draft one');
  assert.equal(stored.drafts.length, 1);
  assert.match(stored.error, /designer crashed/);
});

test('a stopped design is not resurrected by a late design generation', async () => {
  const previous = { revision: 1, yaml: 'draft one', hats: [] };
  const setup = fixture({ runs: table([run({ status: 'stopped', draft: previous, drafts: [previous] })]) });
  const live = { controller: new AbortController(), handles: new Set(), token: 'token', invocations: new Set() };
  await assert.rejects(setup.service.generate('design-one', live), { code: 'STALE_REVISION' });
  const stored = setup.runs.get('design-one');
  assert.equal(stored.status, 'stopped');
  assert.equal(stored.draft.revision, 1);
  assert.equal(stored.drafts.length, 1);
});

test('accept remains user-only even for a restored draft', async () => {
  const setup = fixture({ runs: table([run({ status: 'interrupted' })]) });
  await setup.service.dispatch({ action: 'restore', runId: 'design-one', yaml: BUILD });
  const result = await assert.rejects(setup.service.dispatch({ action: 'accept', runId: 'design-one', revision: 1, name: 'My config' }), { code: 'USER_ONLY' });
  assert.equal(setup.runs.get('design-one').status, 'review', 'accept must not change status on rejection');
});
