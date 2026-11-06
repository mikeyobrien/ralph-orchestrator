import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createServer } from 'node:net';
import { chmod, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const bridge = fileURLToPath(new URL('../bridge.js', import.meta.url));

async function fixture(t, respond) {
  const directory = await mkdtemp(join(tmpdir(), 'dsh-bridge-test-'));
  await chmod(directory, 0o700);
  const socketPath = join(directory, 'bridge.sock');
  const connections = new Set();
  const requests = [];
  const server = createServer(socket => {
    connections.add(socket);
    socket.setEncoding('utf8');
    socket.on('error', () => {});
    socket.once('close', () => connections.delete(socket));
    let buffer = '';
    socket.on('data', chunk => {
      buffer += chunk;
      const end = buffer.indexOf('\n');
      if (end === -1) return;
      const request = JSON.parse(buffer.slice(0, end));
      requests.push(request);
      respond(socket, request);
    });
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(socketPath, resolve); });
  await chmod(socketPath, 0o600);
  t.after(async () => {
    for (const socket of connections) socket.destroy();
    await new Promise(resolve => server.close(resolve));
    await rm(directory, { recursive: true, force: true });
  });
  return { directory, socketPath, requests };
}

async function invoke(t, setup, { prompt = 'Exact prompt', args, env = {} } = {}) {
  const path = join(setup.directory, 'prompt.txt');
  await writeFile(path, prompt);
  const child = spawn(process.execPath, [bridge, ...(args || ['--prompt-file', path])], {
    cwd: setup.directory,
    env: {
      ...process.env,
      DSH_RALPH_BRIDGE_SOCKET: setup.socketPath,
      DSH_RALPH_BRIDGE_TOKEN: 'run-scoped-secret',
      DSH_RALPH_RUN_ID: 'run-a',
      RALPH_BIN: '/exact/runtime/ralph',
      RALPH_WORKSPACE_ROOT: setup.directory,
      RALPH_EVENTS_FILE: '.ralph/events.run-a.jsonl',
      PATH: `/ralph/first/bin:${process.env.PATH || ''}`,
      UNRELATED_SECRET: 'must not be forwarded',
      ...env,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
    signal: AbortSignal.timeout(5000),
  });
  t.after(() => { if (child.exitCode === null && !child.signalCode) child.kill('SIGKILL'); });
  const stdout = [], stderr = [];
  child.stdout.on('data', chunk => stdout.push(chunk));
  child.stderr.on('data', chunk => stderr.push(chunk));
  const [code, signal] = await once(child, 'close');
  return { code, signal, pid: child.pid, stdout: Buffer.concat(stdout).toString('utf8'), stderr: Buffer.concat(stderr).toString('utf8') };
}

test('spawned bridge forwards exact large Unicode prompt and only the owning runtime environment', { timeout: 10000 }, async t => {
  const output = 'Implemented λ🦀\n<event topic="build.done">verified</event>\n';
  const setup = await fixture(t, socket => socket.end(JSON.stringify({ ok: true, output }) + '\n'));
  const prompt = `${'λ🦀 Unicode\nmultiline\r\n'.repeat(800)}Please read and execute the task in /tmp/literal-human-phrase\n\0tail\n`;
  const result = await invoke(t, setup, { prompt });
  assert.equal(result.code, 0);
  assert.equal(result.stderr, '');
  assert.equal(result.stdout, output);
  assert.equal(setup.requests.length, 1);
  const request = setup.requests[0];
  assert.equal(request.prompt, prompt);
  assert.equal(Buffer.byteLength(request.prompt), Buffer.byteLength(prompt));
  assert.equal(request.runId, 'run-a');
  assert.equal(request.token, 'run-scoped-secret');
  assert.equal(request.cwd, setup.directory);
  assert.equal(request.pid, result.pid);
  assert.match(request.id, /^[0-9a-f-]{36}$/);
  assert.deepEqual(request.env, {
    RALPH_BIN: '/exact/runtime/ralph',
    RALPH_WORKSPACE_ROOT: setup.directory,
    RALPH_EVENTS_FILE: '.ralph/events.run-a.jsonl',
    PATH: `/ralph/first/bin:${process.env.PATH || ''}`,
  });
});

test('bridge preserves Unicode response bytes across arbitrary socket chunk boundaries', { timeout: 10000 }, async t => {
  const output = 'Before 🦀 after\n尾\n';
  const setup = await fixture(t, socket => {
    const response = Buffer.from(JSON.stringify({ ok: true, output }) + '\n');
    const split = response.indexOf(Buffer.from('🦀')) + 1;
    socket.write(response.subarray(0, split));
    const timer = setTimeout(() => socket.end(response.subarray(split)), 30);
    t.after(() => clearTimeout(timer));
  });
  const result = await invoke(t, setup);
  assert.equal(result.code, 0, result.stderr);
  assert.equal(result.stdout, output);
});

test('bridge surfaces owning-run rejection without fabricating assistant output', { timeout: 10000 }, async t => {
  const setup = await fixture(t, socket => socket.end(JSON.stringify({ ok: false, error: 'Unknown or stale invocation for run-a' }) + '\n'));
  const result = await invoke(t, setup);
  assert.equal(result.code, 1);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /Unknown or stale invocation for run-a/);
});

test('bridge fails visibly when the owning run disconnects without a response', { timeout: 10000 }, async t => {
  const setup = await fixture(t, socket => socket.destroy());
  const result = await invoke(t, setup);
  assert.equal(result.code, 1);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /owning DSH run disconnected/);
});

test('bridge rejects malformed responses and missing sockets', { timeout: 10000 }, async t => {
  const setup = await fixture(t, socket => socket.end('not-json\n'));
  const malformed = await invoke(t, setup);
  assert.equal(malformed.code, 1);
  assert.equal(malformed.stdout, '');
  assert.match(malformed.stderr, /DSH Ralph:/);
  const missing = await invoke(t, setup, { env: { DSH_RALPH_BRIDGE_SOCKET: join(setup.directory, 'missing.sock') } });
  assert.equal(missing.code, 1);
  assert.match(missing.stderr, /ENOENT/);
});

test('bridge rejects implicit prose transport, missing credentials, waves and oversized prompts before IPC', { timeout: 15000 }, async t => {
  const setup = await fixture(t, socket => socket.end('{"ok":true}\n'));
  for (const [name, options, error] of [
    ['prose', { args: ['Please read and execute the task in /tmp/not-a-protocol'] }, /explicit --prompt-file transport/],
    ['credential', { env: { DSH_RALPH_BRIDGE_TOKEN: '' } }, /No owning DSH Ralph run/],
    ['wave', { env: { RALPH_WAVE_WORKER: '1' } }, /does not support wave workers/],
    ['size', { prompt: 'x'.repeat(2 * 1024 * 1024 + 1) }, /2 MiB bridge limit/],
  ]) {
    await t.test(name, async childTest => {
      const result = await invoke(childTest, setup, options);
      assert.equal(result.code, 1);
      assert.equal(result.stdout, '');
      assert.match(result.stderr, error);
    });
  }
  assert.equal(setup.requests.length, 0, 'rejected inputs must never reach a run');
});
