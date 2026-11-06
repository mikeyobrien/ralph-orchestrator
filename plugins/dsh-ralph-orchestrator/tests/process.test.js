import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { RalphProcess, runCommand } from '../process.js';

const rpcPrelude = `
  import { createInterface } from 'node:readline';
  const send = event => process.stdout.write(JSON.stringify(event) + '\\n');
  const input = createInterface({ input: process.stdin });
  const reply = (request, data) => send({type:'response', command:request.type, id:request.id, success:true, data});
`;

async function launch(t, script, { executable = process.execPath, env = {} } = {}) {
  const cwd = await mkdtemp(join(tmpdir(), 'dsh-rpc-test-'));
  const events = [];
  const child = new RalphProcess({ executable, args: ['--input-type=module', '-e', rpcPrelude + script], cwd, env: { ...process.env, ...env }, onEvent: event => events.push(event) });
  t.after(async () => {
    child.signal('SIGKILL');
    await child.finished;
    await rm(cwd, { recursive: true, force: true });
  });
  return { child, events, cwd };
}

test('RPC correlates concurrent out-of-order replies and preserves terminal evidence', { timeout: 10000 }, async t => {
  const { child, events } = await launch(t, `
    const queued = [];
    input.on('line', line => {
      const request = JSON.parse(line);
      queued.push(request);
      if (queued.length !== 2) return;
      send({type:'response', command:'get_state', id:'not-a-request', success:true, data:{wrong:true}});
      for (const request of queued.reverse()) reply(request, {request:request.type, label:request.label});
      send({type:'loop_terminated', reason:'completed', total_iterations:2, duration_ms:12, total_cost_usd:0, terminated_at:123});
      input.close();
      process.stdin.destroy();
    });
  `);
  const results = await Promise.all([
    child.request('get_state', { label: 'state 🦀' }),
    child.request('get_iterations', { label: 'iterations\nsecond line' }),
  ]);
  assert.deepEqual(results, [
    { request: 'get_state', label: 'state 🦀' },
    { request: 'get_iterations', label: 'iterations\nsecond line' },
  ]);
  const finished = await child.finished;
  assert.equal(finished.code, 0);
  assert.equal(finished.terminal.reason, 'completed');
  assert.equal(finished.terminal.total_iterations, 2);
  assert.ok(events.some(event => event.type === 'loop_terminated'));
});

test('terminal evidence closes the control input so an EOF-waiting RPC child can exit', { timeout: 10000 }, async t => {
  const { child, cwd } = await launch(t, `
    import { writeFileSync } from 'node:fs';
    input.on('line', line => {
      reply(JSON.parse(line), {running:false});
      send({type:'loop_terminated', reason:'completed', total_iterations:1});
    });
    input.on('close', () => writeFileSync('stdin-ended', 'yes'));
  `);
  await child.request('get_state');
  let timer;
  const finished = await Promise.race([child.finished, new Promise(resolve => { timer = setTimeout(() => resolve(null), 700); })]);
  clearTimeout(timer);
  assert.notEqual(finished, null, 'the completed RPC process must not wait forever for frontend stdin');
  assert.equal(finished.code, 0);
  assert.equal(await readFile(join(cwd, 'stdin-ended'), 'utf8'), 'yes');
});

test('RPC surfaces correlated errors without breaking subsequent requests', { timeout: 10000 }, async t => {
  const { child } = await launch(t, `
    input.on('line', line => {
      const request = JSON.parse(line);
      if (request.type === 'set_hat') send({type:'response', command:request.type, id:request.id, success:false, error:'not yet implemented'});
      else reply(request, {hat:'reviewer', iteration:3});
    });
  `);
  await assert.rejects(child.request('set_hat', { hat: 'builder' }), /not yet implemented/);
  assert.deepEqual(await child.request('get_state'), { hat: 'reviewer', iteration: 3 });
});

test('process exit rejects every in-flight request and is not classified as completion', { timeout: 10000 }, async t => {
  const { child } = await launch(t, `
    process.stderr.write('backend could not start\\n');
    input.on('line', line => {
      if (JSON.parse(line).type === 'get_iterations') process.exit(7);
    });
  `);
  const first = assert.rejects(child.request('get_state'), /exited before replying/);
  const second = assert.rejects(child.request('get_iterations'), /exited before replying/);
  await Promise.all([first, second]);
  const finished = await child.finished;
  assert.equal(finished.code, 7);
  assert.equal(finished.terminal, undefined);
  assert.match(finished.stderr, /backend could not start/);
  await assert.rejects(child.request('get_state'), /closed/);
});

test('RPC request deadlines reject a silent live child', { timeout: 10000 }, async t => {
  const { child } = await launch(t, `input.on('line', () => {});`);
  await assert.rejects(child.request('get_state', {}, 100), /get_state timed out/);
});

test('stop sends abort over the real protocol and waits for interrupted process exit', { timeout: 10000 }, async t => {
  const { child, cwd } = await launch(t, `
    import { writeFileSync } from 'node:fs';
    input.on('line', line => {
      const request = JSON.parse(line);
      if (request.type === 'get_state') return reply(request, {running:true});
      writeFileSync('abort.json', JSON.stringify(request));
      reply(request, null);
      send({type:'loop_terminated', reason:'interrupted', total_iterations:1, duration_ms:1, total_cost_usd:0, terminated_at:123});
      input.close();
      process.stdin.destroy();
    });
  `);
  assert.deepEqual(await child.request('get_state'), { running: true });
  const finished = await child.stop();
  const abort = JSON.parse(await readFile(join(cwd, 'abort.json'), 'utf8'));
  assert.equal(abort.type, 'abort');
  assert.equal(abort.reason, 'Stopped from DSH');
  assert.equal(typeof abort.id, 'string');
  assert.equal(finished.code, 0);
  assert.equal(finished.terminal.reason, 'interrupted');
});

test('stop terminates a process that acknowledges abort but does not exit', { timeout: 10000 }, async t => {
  const { child } = await launch(t, `
    input.on('line', line => reply(JSON.parse(line), {running:true}));
  `);
  await child.request('get_state');
  const finished = await child.stop();
  assert.equal(finished.signal, 'SIGTERM');
  assert.equal(finished.terminal, undefined);
});

test('spawn failure settles the owned process with the concrete executable error', { timeout: 10000 }, async t => {
  const { child } = await launch(t, '', { executable: '/definitely/missing/dsh-ralph-test-executable' });
  const finished = await child.finished;
  assert.match(finished.error, /ENOENT/);
  assert.equal(finished.terminal, undefined);
});

test('runCommand observes actual cwd/env/output and reports nonzero exit', { timeout: 10000 }, async t => {
  const cwd = await mkdtemp(join(tmpdir(), 'dsh-command-test-'));
  t.after(() => rm(cwd, { recursive: true, force: true }));
  const result = await runCommand(process.execPath, ['-e', 'console.log(JSON.stringify({cwd:process.cwd(),value:process.env.RUN_SCOPED}));console.error("diagnostic");'], { cwd, env: { RUN_SCOPED: 'iteration λ' } });
  assert.deepEqual(JSON.parse(result.stdout), { cwd, value: 'iteration λ' });
  assert.match(result.stderr, /diagnostic/);
  await assert.rejects(runCommand(process.execPath, ['-e', 'console.error("invalid native config");process.exit(9)']), error => error.code === 'PROCESS_FAILED' && error.exitCode === 9 && /invalid native config/.test(error.message));
});

test('runCommand cancellation settles a real running child', { timeout: 10000 }, async () => {
  await assert.rejects(runCommand(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { signal: AbortSignal.timeout(100) }), error => error.name === 'AbortError');
});
