import test from 'node:test';
import assert from 'node:assert/strict';
import { access, mkdtemp, mkdir, readFile, readdir, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { delimiter, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { stringify } from 'yaml';
import { RalphProcess, runCommand } from '../process.js';

const nativeRalph = process.env.RALPH_TEST_BIN || fileURLToPath(new URL('../../../target/debug/ralph', import.meta.url));

async function bounded(promise, milliseconds, message) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(message())), milliseconds); })]);
  } finally {
    clearTimeout(timer);
  }
}

// This is intentionally a deterministic native adapter, not a DSH/model session
// substitute. It reads the actual prompt file and calls the real emitting CLI;
// stdout contains no completion promise that could activate a text fallback.
const backend = String.raw`
import assert from 'node:assert/strict';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
const args = process.argv.slice(2);
assert.equal(args.length, 2);
assert.equal(args[0], '--prompt-file');
const prompt = readFileSync(args[1]);
const receivedPath = join(process.env.NATIVE_RECORD_DIRECTORY, 'received-' + process.pid + '.bin');
writeFileSync(receivedPath, prompt);
const env = Object.fromEntries(['RALPH_BIN', 'RALPH_WORKSPACE_ROOT', 'RALPH_EVENTS_FILE', 'PATH'].map(key => {
  assert.equal(typeof process.env[key], 'string', key + ' must be supplied by the actual adapter');
  return [key, process.env[key]];
}));
const emissions = [];
for (const [topic, payload] of [['review.passed', 'Native prompt bytes verified'], ['LOOP_COMPLETE', 'Native RPC transport complete']]) {
  const result = spawnSync(env.RALPH_BIN, ['emit', topic, payload], { cwd: process.cwd(), env: process.env, encoding: 'utf8', timeout: 5000 });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  emissions.push({topic, payload, status: result.status});
}
appendFileSync(join(process.env.NATIVE_RECORD_DIRECTORY, 'invocations.jsonl'), JSON.stringify({cwd:process.cwd(), promptFile:args[1], receivedPath, env, emissions}) + '\n');
process.stdout.write('Deterministic backend finished; completion is recorded in the native events file.\n');
`;

test('real native RPC records file-mode completion and exits after terminal stdin EOF', { timeout: 25000 }, async t => {
  const callerCwd = process.cwd();
  const root = await mkdtemp(join(tmpdir(), 'dsh-native-rpc-'));
  let processHandle;
  t.after(async () => {
    try {
      if (processHandle) {
        processHandle.signal('SIGKILL');
        await bounded(processHandle.finished, 3000, () => 'native Ralph did not settle after cleanup SIGKILL');
      }
    } finally {
      assert.ok(root.startsWith(join(tmpdir(), 'dsh-native-rpc-')), 'cleanup stays within the created fixture');
      await rm(root, { recursive: true, force: true });
    }
  });
  const source = join(root, 'source');
  const workspace = join(root, 'run-worktree');
  const records = join(root, 'recordings');
  await mkdir(source);
  await mkdir(records);
  const original = Buffer.from('Source checkout remains independent λ🦀\n');
  await writeFile(join(source, 'source.txt'), original);
  const git = args => runCommand('git', ['-C', source, ...args], { signal: AbortSignal.timeout(5000) });
  await git(['init', '-q']);
  await git(['add', 'source.txt']);
  await git(['-c', 'user.name=Native Transport Test', '-c', 'user.email=native-test@example.invalid', '-c', 'core.hooksPath=/dev/null', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'Isolated fixture']);
  const sourceHead = (await git(['rev-parse', 'HEAD'])).stdout.trim();
  await git(['worktree', 'add', '--detach', workspace, 'HEAD']);
  const executable = await realpath(nativeRalph);
  const script = join(root, 'deterministic-backend.mjs');
  await writeFile(script, backend);
  const configPath = join(root, 'run.yml');
  await writeFile(configPath, stringify({
    cli: { backend: 'custom', command: process.execPath, args: [script], prompt_mode: 'file', prompt_flag: '--prompt-file', default_mode: 'autonomous' },
    event_loop: { max_iterations: 2, completion_promise: 'LOOP_COMPLETE', required_events: ['review.passed'] },
    memories: { enabled: false },
    tasks: { enabled: false },
    features: { auto_merge: false },
    hats: {},
  }));
  const objective = `Preserve the complete original objective exactly:\n${'λ🦀 Unicode objectives\nsecond line\r\n'.repeat(600)}Please read and execute the task in /tmp/this-is-literal-human-text\n終わり\n`;
  assert.ok(Buffer.byteLength(objective) > 7 * 1024);
  // Do not inherit another run's event/config/diagnostic variables. Runtime
  // ownership must be supplied by this native process, not by the test harness.
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('RALPH_')));
  Object.assign(env, { PATH: `/home/mobrienv/.cargo/bin:${process.env.PATH || ''}`, RALPH_DIAGNOSTICS: '0', NATIVE_RECORD_DIRECTORY: records });
  const events = [];
  processHandle = new RalphProcess({
    executable,
    args: ['-c', configPath, 'run', '--rpc', '--no-auto-merge', '--skip-preflight', '-p', objective, '--max-iterations', '2'],
    cwd: workspace,
    env,
    onEvent: event => events.push(event),
  });
  const finished = await bounded(processHandle.finished, 15000, () => `native RPC did not exit after terminal evidence: ${JSON.stringify(events.filter(event => event.type === 'loop_terminated'))}; stderr=${processHandle.stderr}`);
  assert.equal(finished.code, 0, finished.stderr);
  assert.equal(finished.signal, null);
  assert.equal(finished.error, undefined);
  assert.equal(finished.terminal?.reason, 'completed', finished.stderr);
  assert.ok(finished.terminal.total_iterations >= 1 && finished.terminal.total_iterations <= 2);
  assert.equal(events.filter(event => event.type === 'loop_terminated').length, 1);
  assert.ok(events.some(event => event.type === 'iteration_start'));
  assert.ok(events.some(event => event.type === 'iteration_end'));

  const invocations = (await readFile(join(records, 'invocations.jsonl'), 'utf8')).trim().split('\n').map(line => JSON.parse(line));
  assert.equal(invocations.length, finished.terminal.total_iterations);
  for (const invocation of invocations) {
    const received = await readFile(invocation.receivedPath);
    assert.ok(received.indexOf(Buffer.from(objective)) !== -1, 'the actual backend prompt must contain every original objective byte, not tempfile prose');
    assert.equal(invocation.cwd, await realpath(workspace));
    assert.equal(invocation.env.RALPH_WORKSPACE_ROOT, invocation.cwd);
    assert.equal(await realpath(invocation.env.RALPH_BIN), executable);
    assert.equal(invocation.env.PATH.split(delimiter)[0], dirname(executable));
    const eventsPath = resolve(invocation.cwd, invocation.env.RALPH_EVENTS_FILE);
    assert.ok(!relative(invocation.cwd, eventsPath).startsWith('..'), 'events belong to the run worktree');
    const nativeEvents = (await readFile(eventsPath, 'utf8')).trim().split('\n').map(line => JSON.parse(line));
    assert.ok(nativeEvents.some(event => event.topic === 'review.passed' && event.payload === 'Native prompt bytes verified'));
    assert.ok(nativeEvents.some(event => event.topic === 'LOOP_COMPLETE' && event.payload === 'Native RPC transport complete'));
    assert.deepEqual(invocation.emissions.map(emission => emission.status), [0, 0]);
    await assert.rejects(access(invocation.promptFile), error => error.code === 'ENOENT');
  }
  assert.equal(process.cwd(), callerCwd, 'launching session cwd remains unchanged');
  assert.equal((await git(['rev-parse', 'HEAD'])).stdout.trim(), sourceHead);
  assert.equal((await git(['status', '--porcelain'])).stdout, '');
  assert.deepEqual((await readdir(source)).sort(), ['.git', 'source.txt']);
  assert.deepEqual(await readFile(join(source, 'source.txt')), original);
});
