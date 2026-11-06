import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

const clientSource = await readFile(new URL('../client.js', import.meta.url), 'utf8');
function deferred() { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; }
const text = node => typeof node === 'string' ? node : Array.isArray(node) ? node.map(text).join('') : node && typeof node === 'object' ? text(node.children) : '';

async function fixture(t, controls = {}) {
  const id = 'cold-persisted-summary';
  const observed = { sequence: [], opened: [], requests: [], currentView: 'ralph-panel' };
  const known = new Set();
  const disposers = [];
  let Panel, plugin;
  const React = {
    Fragment: Symbol('fragment'),
    createElement(type, props, ...children) { return { type, props: { ...(props || {}), children: children.length === 1 ? children[0] : children } }; },
    useSyncExternalStore(_subscribe, snapshot) { return snapshot(); },
  };
  const render = node => {
    if (node === null || node === undefined || typeof node === 'boolean') return null;
    if (Array.isArray(node)) return node.map(render);
    if (typeof node !== 'object') return node;
    if (typeof node.type === 'function') return render(node.type(node.props));
    return { type: node.type, props: node.props, children: render(node.props.children) };
  };
  const all = (node, predicate) => {
    if (!node || typeof node !== 'object') return [];
    if (Array.isArray(node)) return node.flatMap(item => all(item, predicate));
    return [...(predicate(node) ? [node] : []), ...all(node.children, predicate)];
  };
  const storage = new Map();
  const ctx = {
    effect(operation) { const off = operation(); if (typeof off === 'function') disposers.push(off); return off; },
    locale: { register: () => () => {}, bind: () => key => key, getLocale: () => ({ active: 'en' }) },
    slots: {
      inject(_name, operation) { return operation(); },
      register(options, component) { if (options.name === 'main') Panel = component; return () => {}; },
    },
    sessions: {
      async refresh() {
        observed.sequence.push('catalog-refresh-start');
        if (controls.gate) await controls.gate;
        if (controls.error) throw controls.error;
        if (!controls.missing) known.add(id);
        observed.sequence.push('catalog-refresh-complete');
      },
    },
    uiWorkspace: {
      openSession(target) {
        if (!known.has(target)) throw new Error(`sessions.retain: unknown session ${target}`);
        observed.sequence.push('navigate');
        observed.opened.push(target);
        observed.currentView = target;
      },
    },
  };
  vm.runInNewContext(clientSource, {
    window: { __ModuleLoader__: { load({ factory }) { plugin = factory(name => { assert.equal(name, 'react'); return React; }); } } },
    sessionStorage: { getItem: key => controls.stored !== undefined ? controls.stored : storage.get(key) || null, setItem: (key, value) => storage.set(key, value) },
    AbortController,
    EventSource: class { addEventListener() {} removeEventListener() {} close() {} },
    fetch: async (url, options) => {
      observed.requests.push({ url, method: options.method || 'GET' });
      return { ok: true, status: 200, json: async () => ({ ok: true, value: {
        workspaces: [], defaults: {},
        configurations: controls.configurations || [],
        runs: [controls.run || { id: 'run-one', name: 'Finished run', mode: 'run', status: 'completed', iteration: 1, maxIterations: 3, prompt: 'Original task', createdAt: 1, sessions: [{ id, title: 'DONE - Summary', status: 'completed' }] }],
      } }) };
    },
  });
  plugin.apply(ctx);
  t.after(() => { for (const dispose of disposers.reverse()) dispose(); });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(typeof Panel, 'function');
  const tree = () => render(React.createElement(Panel, {}));
  const sessionLink = () => {
    const links = all(tree(), node => node.type === 'button' && text(node) === 'DONE - Summary');
    assert.equal(links.length, 1, 'activate the real rendered session link, not a copied helper');
    return links[0];
  };
  const runCard = label => all(tree(), node => node.props && node.props['aria-label'] === label).map(text).join('\n');
  return { id, observed, sessionLink, runCard, card: () => text(tree()), alerts: () => all(tree(), node => node.props.role === 'alert').map(text), find: predicate => all(tree(), predicate) };
}

test('rendered cold-session link refreshes the public catalog before synchronous navigation', async t => {
  const gate = deferred();
  const setup = await fixture(t, { gate: gate.promise });
  const clicking = setup.sessionLink().props.onClick();
  assert.deepEqual(setup.observed.sequence, ['catalog-refresh-start']);
  assert.equal(setup.observed.currentView, 'ralph-panel');
  assert.deepEqual(setup.observed.opened, []);
  gate.resolve(); await clicking;
  assert.deepEqual(setup.observed.sequence, ['catalog-refresh-start', 'catalog-refresh-complete', 'navigate']);
  assert.deepEqual(setup.observed.opened, [setup.id]);
  assert.equal(setup.observed.currentView, setup.id);
  assert.deepEqual(setup.alerts(), []);
  assert.deepEqual(setup.observed.requests, [{ url: '/api/ralph-orchestrator', method: 'GET' }]);
});

test('catalog rejection preserves the current panel, renders the failure and a later successful open clears it', async t => {
  const controls = { error: new Error('Session catalog refresh failed') };
  const setup = await fixture(t, controls);
  await setup.sessionLink().props.onClick();
  assert.equal(setup.observed.currentView, 'ralph-panel'); assert.deepEqual(setup.observed.opened, []);
  assert.ok(setup.alerts().some(message => message.includes('Session catalog refresh failed')));
  delete controls.error;
  await setup.sessionLink().props.onClick();
  assert.deepEqual(setup.observed.opened, [setup.id]);
  assert.deepEqual(setup.alerts(), []);
});

test('a resolved refresh without the target still surfaces the navigation guard without changing views', async t => {
  const setup = await fixture(t, { missing: true });
  await setup.sessionLink().props.onClick();
  assert.deepEqual(setup.observed.sequence, ['catalog-refresh-start', 'catalog-refresh-complete']);
  assert.deepEqual(setup.observed.opened, []); assert.equal(setup.observed.currentView, 'ralph-panel');
  assert.ok(setup.alerts().some(message => message.includes(`sessions.retain: unknown session ${setup.id}`)));
});

test('a run card shows the configuration that ran, including the YAML Ralph executed', async t => {
  const setup = await fixture(t, {
    configurations: [{ id: 'builtin:build-review', name: 'Build and review', revision: 1, yaml: 'approved source', hats: [] }],
    run: {
      id: 'run-one', name: 'Finished run', mode: 'run', status: 'completed', iteration: 1, maxIterations: 3, prompt: 'Original task', createdAt: 1,
      configurationId: 'builtin:build-review', configurationRevision: 1, hats: [],
      executionYaml: 'cli: { backend: custom }', sessions: [{ id: 'cold-persisted-summary', title: 'DONE - Summary', status: 'completed' }],
    },
  });
  const rendered = setup.runCard('Finished run');
  assert.match(rendered, /configuration: Build and review/);
  assert.match(rendered, /cli: \{ backend: custom \}/);
  assert.doesNotMatch(rendered, /approved source/);
});

test('an interrupted design without a draft renders a YAML paste-and-restore affordance', async t => {
  const setup = await fixture(t, {
    stored: JSON.stringify({ version: 1, mode: 'design', design: {}, reviews: [] }),
    run: { id: 'design-one', name: 'Design task', mode: 'design', status: 'interrupted', iteration: 1, maxIterations: 3, prompt: 'Workflow requirements', createdAt: 1, drafts: [], sessions: [] },
  });
  const rendered = setup.runCard('Design task');
  assert.match(rendered, /restoreYaml/);
  assert.match(rendered, /restoreHint/);
  assert.equal(setup.find(node => node.type === 'textarea' && node.props.rows === 8).length, 1, 'the user must be able to paste a configuration');
  const restoreButton = setup.find(node => node.type === 'button' && text(node) === 'restore');
  assert.equal(restoreButton.length, 1, 'the rendered restore action must be present');
  assert.doesNotMatch(rendered, /resume/i, 'the deferred resume feature has no prominent GUI affordance');
});
