import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { stringify } from 'yaml';
import { BUILTINS, executionConfiguration, inspectHats, iterationTitle, readConfiguration, shortName } from '../configuration.js';
import { runCommand } from '../process.js';
import { RalphOrchestrator } from '../index.js';

const ralph = process.env.RALPH_TEST_BIN || fileURLToPath(new URL('../../../target/debug/ralph', import.meta.url));
const options = { bridgePath: '/private/run/bridge.js', executable: process.execPath, maxIterations: 7 };

function workflow() {
  return readConfiguration(BUILTINS.find(config => config.id === 'builtin:build-review').yaml);
}

test('execution overlay preserves native hats, routing, instructions and unrelated configuration', () => {
  const original = workflow();
  original.cli = { backend: 'claude', args: ['--old'], default_mode: 'autonomous', idle_timeout_secs: 45 };
  original.event_loop.required_events = ['review.passed'];
  original.event_loop.max_runtime_seconds = 300;
  original.memories = { enabled: true, inject: 'auto' };
  original.tasks = { enabled: true };
  original.features = { auto_merge: true, custom_feature: true };
  original.hats.builder.extra_instructions = ['Keep existing behavior', 'Record the real test output'];
  const yaml = stringify(original);
  const result = readConfiguration(executionConfiguration(yaml, options));
  assert.deepEqual(result.hats, original.hats);
  assert.deepEqual(result.event_loop, { ...original.event_loop, max_iterations: 7 });
  assert.deepEqual(result.memories, original.memories);
  assert.deepEqual(result.tasks, original.tasks);
  assert.deepEqual(result.features, { auto_merge: false, custom_feature: true });
  assert.deepEqual(result.cli, { ...original.cli, backend: 'custom', command: process.execPath, args: ['/private/run/bridge.js'], prompt_mode: 'file', prompt_flag: '--prompt-file' });
  assert.equal(readConfiguration(yaml).features.auto_merge, true, 'the accepted source remains unchanged');
});

test('hat inspection shows native details and instruction fragments without executing them', () => {
  const original = workflow();
  original.hats.builder.instructions = 'Main implementation instructions';
  original.hats.builder.extra_instructions = ['Run the real checks', 'Preserve source state'];
  const hats = inspectHats(stringify(original));
  assert.deepEqual(hats.find(hat => hat.id === 'builder'), {
    id: 'builder', name: 'Builder', description: original.hats.builder.description,
    triggers: ['build.start', 'review.rejected'], publishes: ['review.requested'],
    instructions: 'Main implementation instructions\n\nRun the real checks\n\nPreserve source state',
    backend: 'dsh',
  });
});

test('hat inspection surfaces the per-hat backend and model without changing the source', () => {
  const original = workflow();
  original.hats.builder.backend = 'codex';
  original.hats.builder.model = 'gpt-6.1-sol';
  original.hats.reviewer.backend = 'claude-code';
  original.hats.reviewer.model = 'claude-opus-5-5';
  const yaml = stringify(original);
  const hats = inspectHats(yaml);
  assert.deepEqual(hats.find(hat => hat.id === 'builder'), {
    id: 'builder', name: 'Builder', description: original.hats.builder.description,
    triggers: ['build.start', 'review.rejected'], publishes: ['review.requested'],
    instructions: original.hats.builder.instructions, backend: 'codex', model: 'gpt-6.1-sol',
  });
  assert.equal(hats.find(hat => hat.id === 'reviewer').backend, 'claude-code');
  assert.equal(readConfiguration(yaml).hats.builder.backend, 'codex', 'inspection never rewrites the source');
});

test('execution overlay accepts per-hat backends and strips them before Ralph validates', () => {
  const original = workflow();
  original.hats.builder.backend = 'codex';
  original.hats.builder.model = 'gpt-6.1-sol';
  original.hats.reviewer.backend = 'claude-code';
  original.hats.reviewer.model = 'claude-opus-5-5';
  const yaml = stringify(original);
  const emitted = executionConfiguration(yaml, options);
  const result = readConfiguration(emitted);
  assert.equal(result.hats.builder.backend, undefined);
  assert.equal(result.hats.builder.model, undefined);
  assert.equal(result.hats.reviewer.backend, undefined);
  assert.equal(result.hats.reviewer.model, undefined);
  assert.equal(result.cli.backend, 'custom');
  assert.deepEqual(result.cli.args, ['/private/run/bridge.js']);
  assert.equal(readConfiguration(yaml).hats.builder.backend, 'codex', 'the accepted source remains unchanged');
});

test('execution overlay keeps backend-unset output byte-identical to the pre-feature bridge', async () => {
  const yaml = BUILTINS.find(config => config.id === 'builtin:build-review').yaml;
  const golden = await readFile(new URL('./fixtures/execution-default.golden.yml', import.meta.url), 'utf8');
  assert.equal(executionConfiguration(yaml, { bridgePath: '/fixed/bridge.js', executable: '/fixed/node', maxIterations: 7 }), golden);
});

test('execution overlay rejects native backend names, custom commands and backend arguments', async t => {
  for (const patch of [
    { backend: 'claude' },
    { backend: 'pi' },
    { backend: { type: 'custom', command: '/bin/echo', args: ['bypass'] } },
    { backend: { type: 'codex' } },
    { backend_args: [] },
    { args: ['--foreign-backend'] },
  ]) {
    await t.test(JSON.stringify(patch), () => {
      const config = workflow();
      Object.assign(config.hats.builder, patch);
      assert.throws(() => executionConfiguration(stringify(config), options), /hats\.builder\.(backend|backend_args)/);
    });
  }
});

test('per-hat models are validated per backend form', () => {
  const dsh = workflow();
  dsh.hats.builder.backend = 'dsh';
  dsh.hats.builder.model = 'sparks/GLM-5.3-Flash-EXL3';
  assert.equal(readConfiguration(executionConfiguration(stringify(dsh), options)).hats.builder.model, undefined);
  for (const model of ['GLM-5.3-Flash-EXL3', 'sparks/', '/GLM', 'a/b/c', '  ']) {
    const config = workflow();
    config.hats.builder.backend = 'dsh';
    config.hats.builder.model = model;
    assert.throws(() => executionConfiguration(stringify(config), options), /hats\.builder\.model/);
  }
  const cli = workflow();
  cli.hats.builder.backend = 'codex';
  cli.hats.builder.model = 'gpt-6.1-terra';
  assert.equal(readConfiguration(executionConfiguration(stringify(cli), options)).hats.builder.model, undefined);
  const invalid = workflow();
  invalid.hats.builder.backend = 'codex';
  invalid.hats.builder.model = 42;
  assert.throws(() => executionConfiguration(stringify(invalid), options), /hats\.builder\.model/);
});

test('execution overlay rejects unsupported waves, aggregation and cost accounting', async t => {
  for (const patch of [{ concurrency: 0 }, { concurrency: 2 }, { aggregate: { mode: 'wait_for_all', timeout: 30 } }]) {
    await t.test(JSON.stringify(patch), () => {
      const config = workflow();
      Object.assign(config.hats.builder, patch);
      assert.throws(() => executionConfiguration(stringify(config), options), /concurrent\/aggregate hats are not supported/);
    });
  }
  for (const cost of [0, 1.5]) {
    const v1 = workflow();
    v1.max_cost = cost;
    assert.throws(() => executionConfiguration(stringify(v1), options), /Cost budgets require backend metrics/);
    const v2 = workflow();
    v2.event_loop.max_cost_usd = cost;
    assert.throws(() => executionConfiguration(stringify(v2), options), /Cost budgets require backend metrics/);
  }
  const sequential = workflow();
  sequential.hats.builder.concurrency = 1;
  assert.equal(readConfiguration(executionConfiguration(stringify(sequential), options)).hats.builder.concurrency, 1);
});

test('configuration reader rejects invalid mappings, duplicate keys and oversized input', () => {
  for (const yaml of ['', '[]', 'text', 'hats: {}\nhats: {}\n', 'hats: [']) {
    assert.throws(() => readConfiguration(yaml));
  }
  assert.throws(() => readConfiguration('x'.repeat(256 * 1024 + 1)), /smaller than 256 KiB/);
});

async function nativeDryRun(t, raw, expectedError) {
  const directory = await mkdtemp(join(tmpdir(), 'dsh-native-config-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const workspace = join(directory, 'workspace');
  await mkdir(workspace);
  const backend = join(directory, 'backend-must-not-run.mjs');
  await writeFile(backend, `import {writeFileSync} from 'node:fs';writeFileSync('BACKEND_EXECUTED','unexpected');process.exit(97);`);
  const path = join(directory, 'candidate.yml');
  await writeFile(path, executionConfiguration(raw, { bridgePath: backend, executable: process.execPath, maxIterations: 3 }));
  const invoke = () => runCommand(ralph, ['-c', path, 'run', '--dry-run', '--skip-preflight', '-p', 'Validate only; do not execute backend'], {
    cwd: workspace, env: { ...process.env, PATH: `/home/mobrienv/.cargo/bin:${process.env.PATH || ''}` }, signal: AbortSignal.timeout(10000),
  });
  if (expectedError) {
    await assert.rejects(invoke(), error => error.code === 'PROCESS_FAILED' && expectedError.test(error.message));
  } else {
    const result = await invoke();
    assert.match(result.stdout, /Dry run mode - configuration:/);
    assert.match(result.stdout, /Max iterations: 3/);
    assert.match(result.stdout, /Backend: custom/);
  }
  // Native TUI-mode startup writes diagnostic logs before the dry-run branch.
  // No backend, event, task, lock or other work artifact is allowed alongside them.
  const entries = await readdir(workspace, { recursive: true });
  const loggingDirectories = new Set(['.ralph', '.ralph/diagnostics', '.ralph/diagnostics/logs']);
  assert.deepEqual(entries.filter(entry => !loggingDirectories.has(entry) && !/^\.ralph\/diagnostics\/logs\/ralph-[\w.-]+\.log$/.test(entry)), [], 'dry-run must produce no work artifacts');
}

test('real native dry-run accepts both shipped workflows without work artifacts', { timeout: 30000 }, async t => {
  for (const builtin of BUILTINS) await t.test(builtin.id, child => nativeDryRun(child, builtin.yaml));
});

test('legacy native selectors cannot override the bridge or requested iteration budget', { timeout: 30000 }, async t => {
  const config = workflow();
  config.agent = 'claude';
  config.max_iterations = 99;
  await nativeDryRun(t, stringify(config));
});

test('real native dry-run rejects reserved triggers before any work executes', { timeout: 30000 }, async t => {
  for (const trigger of ['task.start', 'task.resume']) {
    await t.test(trigger, async child => {
      const config = workflow();
      config.hats.builder.triggers = [trigger];
      await nativeDryRun(child, stringify(config), /Reserved trigger/);
    });
  }
});

test('real native dry-run rejects missing and blank descriptions without work artifacts', { timeout: 30000 }, async t => {
  for (const description of [undefined, '', ' \t\n ']) {
    await t.test(JSON.stringify(description) || 'missing', async child => {
      const config = workflow();
      if (description === undefined) delete config.hats.builder.description;
      else config.hats.builder.description = description;
      await nativeDryRun(child, stringify(config), /description/i);
    });
  }
});

test('service native validation leaves the actual source directory unchanged on success and failure', { timeout: 30000 }, async t => {
  const directory = await mkdtemp(join(tmpdir(), 'dsh-service-validate-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const source = join(directory, 'source');
  await mkdir(source);
  const sentinel = Buffer.from('Original source bytes λ🦀\n\0tail');
  await writeFile(join(source, 'source.txt'), sentinel);
  const service = new RalphOrchestrator(null, {});
  service.executable = ralph;
  const yaml = BUILTINS.find(config => config.id === 'builtin:build-review').yaml;
  const result = await service.validate(yaml, 5, source);
  assert.equal(result.yaml, yaml);
  assert.deepEqual(result.hats, inspectHats(yaml));
  const prepared = readConfiguration(result.prepared);
  assert.equal(prepared.cli.backend, 'custom');
  assert.equal(prepared.event_loop.max_iterations, 5);
  assert.deepEqual(await readdir(source, { recursive: true }), ['source.txt']);
  const invalid = workflow();
  invalid.hats.builder.triggers = ['task.start'];
  await assert.rejects(service.validate(stringify(invalid), 5, source), /Reserved trigger/);
  assert.deepEqual(await readdir(source, { recursive: true }), ['source.txt']);
  assert.deepEqual(await readFile(join(source, 'source.txt')), sentinel);
});

test('workspace slugs and iteration names are bounded and retain visible iteration identity', () => {
  assert.equal(shortName('  Fix\nRPC / exact bytes !! '), 'fix-rpc-exact-bytes');
  assert.equal(shortName('///🦀///'), '');
  assert.match(shortName('A useful task '.repeat(20)), /^[a-z0-9](?:[a-z0-9-]{0,46}[a-z0-9])?$/);
  assert.equal(iterationTitle(2, 10, 'Reviewer', 'Verify exact prompt'), '2/10 [Reviewer] - Verify exact prompt');
  assert.equal(iterationTitle(3, 10, 'Builder', '\n\t'), '3/10 [Builder] - Work on objective');
  const title = iterationTitle(12, 50, 'Reviewer\n\x00with tabs\t', 'Long title '.repeat(100));
  assert.ok(title.startsWith('12/50 [Reviewer with tabs] - '));
  assert.doesNotMatch(title, /[\x00-\x1f\x7f]/);
  assert.ok(title.length <= 12 + 36 + 72);
});

test('a launch records the exact YAML Ralph was handed', { timeout: 30000 }, async t => {
  const directory = await mkdtemp(join(tmpdir(), 'dsh-service-launch-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const service = new RalphOrchestrator(null, {});
  service.executable = ralph;
  service.update = async (id, patch) => Object.assign(service.runs.get(id), typeof patch === 'function' ? patch() : patch);
  service.isolate = async () => {};
  const yaml = BUILTINS.find(config => config.id === 'builtin:build-review').yaml;
  const run = { id: 'run-config', name: 'proj-task', taskName: 'task', yaml, hats: inspectHats(yaml), maxIterations: 5, mode: 'run', path: directory, prompt: 'objective', permissionPreset: 'default' };
  service.runs = { put() {}, get: id => (id === run.id ? run : undefined) };
  service.requireRun = id => service.runs.get(id);
  await service.launch(run.id, { controller: new AbortController(), handles: new Set(), token: 't', invocations: new Set(), process: null }).catch(() => {});
  assert.equal(run.executionYaml, await readFile(run.configPath, 'utf8'));
  const executed = readConfiguration(run.executionYaml);
  assert.equal(executed.cli.backend, 'custom');
  assert.equal(executed.event_loop.max_iterations, 5);
  assert.deepEqual(run.hats, inspectHats(yaml));
});
