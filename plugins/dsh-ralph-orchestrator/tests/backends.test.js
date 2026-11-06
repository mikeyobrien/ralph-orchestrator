import test from 'node:test';
import assert from 'node:assert/strict';
import { chmod, mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  HAT_BACKENDS, backendError, collectSecrets, hatBackend, parseAgentModel, parseHatBackend,
  redact, resolveAgentSdk, resolveCodexExecutable, runClaudeIteration, runCodexIteration,
} from '../backends.js';

const fakeCodex = fileURLToPath(new URL('./fixtures/fake-codex-app-server.mjs', import.meta.url));
await chmod(fakeCodex, 0o755);

async function directory(t) {
  const path = await mkdtemp(join(tmpdir(), 'ralph-backend-test-'));
  t.after(() => rm(path, { recursive: true, force: true }));
  return path;
}

async function traceEvents(path) {
  const text = await readFile(path, 'utf8').catch(() => '');
  return text.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
}

async function codexCase(t, { scenario = 'ok', prompt = 'Exact native Ralph assembled prompt λ🦀\n\n<hat>Keep this byte-for-byte</hat>\r\n', model = 'gpt-6.1-sol', timeoutMs = 15000, env = {}, signal } = {}) {
  const workdir = await directory(t);
  const trace = join(workdir, 'trace.jsonl');
  const logPath = join(workdir, 'backends', 'iteration.log');
  const args = { command: fakeCodex, cwd: workdir, prompt, model, timeoutMs, signal, logPath, env: { ...process.env, FAKE_CODEX_SCENARIO: scenario, FAKE_CODEX_TRACE: trace, ...env } };
  return { workdir, trace, logPath, prompt, args, events: async () => traceEvents(trace) };
}

test('codex iteration spawns codex app-server --stdio in the run worktree with the exact hat prompt', async t => {
  const run = await codexCase(t);
  const result = await runCodexIteration(run.args);
  const events = await run.events();
  assert.deepEqual(events.find(event => event.kind === 'spawn').argv, ['app-server', '--stdio']);
  assert.equal(events.find(event => event.kind === 'spawn').cwd, run.workdir);
  const catalog = events.find(event => event.kind === 'catalog');
  assert.equal(catalog.includeHidden, true);
  assert.deepEqual(catalog.ids, ['gpt-6.1-sol', 'gpt-6.1-terra', 'gpt-6-astra']);
  const thread = events.find(event => event.kind === 'thread');
  assert.equal(thread.cwd, run.workdir);
  assert.equal(thread.ephemeral, true);
  assert.equal(thread.model, 'gpt-6.1-sol');
  assert.equal(thread.approvalPolicy, 'never');
  const turn = events.find(event => event.kind === 'turn');
  assert.equal(turn.threadId, 'th_fake');
  assert.deepEqual(turn.input, [{ type: 'text', text: run.prompt, text_elements: [] }]);
  assert.equal(result.output, 'fake codex final answer');
  assert.equal(result.exitCode, 0);
  assert.equal(result.logPath, run.logPath);
  const log = await readFile(result.logPath, 'utf8');
  assert.match(log, /backend: codex/);
  assert.match(log, /model: gpt-6\.1-sol/);
  assert.ok(log.includes(run.prompt), 'the captured log retains the exact mapped prompt');
});

test('codex final answer wins over an earlier partial draft', async t => {
  const run = await codexCase(t, { scenario: 'partial' });
  const result = await runCodexIteration(run.args);
  assert.equal(result.output, 'fake codex final answer');
});

test('codex tolerates non-frame stdout diagnostics without losing the terminal answer', async t => {
  const run = await codexCase(t, { scenario: 'noise' });
  const result = await runCodexIteration(run.args);
  assert.equal(result.output, 'fake codex final answer');
});

test('codex declines server-initiated approvals instead of waiting for a human', async t => {
  const run = await codexCase(t, { scenario: 'approval' });
  const result = await runCodexIteration(run.args);
  const decision = (await run.events()).find(event => event.kind === 'response' && event.id === 'srv-1');
  assert.deepEqual(decision.result, { decision: 'decline' });
  assert.equal(result.output, 'approved request handled');
});

test('codex refuses a model the resolved CLI does not offer and reports its catalog', async t => {
  const run = await codexCase(t, { scenario: 'catalog-missing', model: 'gpt-6.1-sol' });
  await assert.rejects(runCodexIteration(run.args), error => {
    assert.equal(error.code, 'MODEL_UNAVAILABLE');
    assert.match(error.message, /gpt-6\.1-sol/);
    assert.match(error.message, /available: gpt-5\.6-sol, gpt-6-astra/);
    return true;
  });
  assert.equal((await run.events()).some(event => event.kind === 'turn'), false, 'no turn is started for a model the CLI does not offer');
});

test('codex failed turn status fails the iteration instead of yielding an answer', async t => {
  const run = await codexCase(t, { scenario: 'error' });
  await assert.rejects(runCodexIteration(run.args), error => {
    assert.equal(error.code, 'TURN_FAILED');
    assert.match(error.message, /status failed/);
    return true;
  });
});

test('codex child exit before the terminal notification fails with captured stderr', async t => {
  const run = await codexCase(t, { scenario: 'exit' });
  await assert.rejects(runCodexIteration(run.args), error => {
    assert.equal(error.code, 'PROTOCOL');
    assert.match(error.message, /fake-codex exploded mid-turn/);
    return true;
  });
});

test('codex timeout kills the app-server child and preserves the log', async t => {
  const run = await codexCase(t, { scenario: 'hang', timeoutMs: 400 });
  await assert.rejects(runCodexIteration(run.args), error => { assert.equal(error.code, 'TIMEOUT'); return true; });
  const pid = (await run.events()).find(event => event.kind === 'spawn').pid;
  assert.throws(() => process.kill(pid, 0), /ESRCH/);
  assert.match(await readFile(run.logPath, 'utf8'), /time limit|timeout/i);
});

test('codex abandons the child when the iteration signal aborts', async t => {
  const controller = new AbortController();
  const run = await codexCase(t, { scenario: 'hang', signal: controller.signal });
  const running = runCodexIteration(run.args);
  setTimeout(() => controller.abort(new Error('stopped from DSH')), 150);
  await assert.rejects(running, error => { assert.equal(error.code, 'ABORTED'); return true; });
  const pid = (await run.events()).find(event => event.kind === 'spawn').pid;
  assert.throws(() => process.kill(pid, 0), /ESRCH/);
});

test('captured logs and results redact secret-shaped values', async t => {
  const run = await codexCase(t, { scenario: 'stderr', env: { OPENAI_API_KEY: 'sk-livesecret123456' } });
  const result = await runCodexIteration(run.args);
  const log = await readFile(result.logPath, 'utf8');
  assert.equal(log.includes('sk-livesecret123456'), false);
  assert.equal(result.stderrTail.includes('sk-livesecret123456'), false);
  assert.match(log, /REDACTED/);
});

function fakeSdk(behaviour = {}) {
  const calls = [];
  return {
    calls,
    query({ prompt, options }) {
      calls.push({ prompt, options });
      if (behaviour.throw) throw new Error('fake SDK exploded during startup');
      if (behaviour.hang) return (async function* () {
        await new Promise((resolve, reject) => options.abortController.signal.addEventListener('abort', () => reject(Object.assign(new Error('aborted'), { name: 'AbortError' })), { once: true }));
      })();
      return (async function* () {
        yield { type: 'system', subtype: 'init', session_id: 'fake', tools: [] };
        yield { type: 'assistant', message: { content: [{ type: 'text', text: 'working notes' }] } };
        yield { type: 'result', subtype: behaviour.subtype || 'success', is_error: Boolean(behaviour.is_error), result: behaviour.result ?? 'fake claude final answer', errors: behaviour.errors || [] };
      })();
    },
  };
}

test('claude iteration drives the Agent SDK with the exact prompt, model, cwd and unattended permissions', async t => {
  const workdir = await directory(t);
  const sdk = fakeSdk();
  const prompt = 'Exact native Ralph assembled prompt λ🦀';
  const result = await runClaudeIteration({ sdk, cwd: workdir, prompt, model: 'claude-opus-5-5', env: { ...process.env }, timeoutMs: 15000, logPath: join(workdir, 'claude.log') });
  assert.equal(sdk.calls.length, 1);
  assert.equal(sdk.calls[0].prompt, prompt);
  const options = sdk.calls[0].options;
  assert.equal(options.cwd, workdir);
  assert.equal(options.model, 'claude-opus-5-5');
  assert.equal(options.persistSession, false);
  assert.equal(options.permissionMode, 'bypassPermissions');
  assert.equal(options.allowDangerouslySkipPermissions, true);
  assert.equal(result.output, 'fake claude final answer');
  assert.match(await readFile(result.logPath, 'utf8'), /backend: claude-code/);
});

test('claude result failures are surfaced as errors, not as an answer', async t => {
  const workdir = await directory(t);
  const sdk = fakeSdk({ subtype: 'error_max_turns', is_error: true, errors: ['turn limit reached'] });
  await assert.rejects(runClaudeIteration({ sdk, cwd: workdir, prompt: 'work', model: 'claude-opus-5-5', timeoutMs: 15000, logPath: join(workdir, 'claude.log') }), error => {
    assert.equal(error.code, 'SDK_FAILURE');
    assert.match(error.message, /error_max_turns/);
    assert.match(error.message, /turn limit reached/);
    return true;
  });
});

test('claude timeout aborts the SDK query and reports a time limit', async t => {
  const workdir = await directory(t);
  const sdk = fakeSdk({ hang: true });
  await assert.rejects(runClaudeIteration({ sdk, cwd: workdir, prompt: 'work', model: 'claude-opus-5-5', timeoutMs: 200, logPath: join(workdir, 'claude.log') }), error => {
    assert.equal(error.code, 'TIMEOUT');
    return true;
  });
  assert.equal(sdk.calls[0].options.abortController.signal.aborted, true);
});

test('claude follows an iteration abort instead of reporting a completed turn', async t => {
  const workdir = await directory(t);
  const sdk = fakeSdk({ hang: true });
  const controller = new AbortController();
  const running = runClaudeIteration({ sdk, cwd: workdir, prompt: 'work', model: 'claude-opus-5-5', timeoutMs: 15000, signal: controller.signal, logPath: join(workdir, 'claude.log') });
  setTimeout(() => controller.abort(new Error('stopped from DSH')), 100);
  await assert.rejects(running, error => { assert.equal(error.code, 'ABORTED'); return true; });
  assert.equal(sdk.calls[0].options.abortController.signal.aborted, true);
});

test('claude SDK resolution reports both attempted locations instead of a vendored copy', async t => {
  const workdir = await directory(t);
  const isolated = join(workdir, 'isolated');
  await mkdir(isolated, { recursive: true });
  await assert.rejects(resolveAgentSdk({ profileDir: undefined, requireBase: join(isolated, 'plugin.js') }), error => {
    assert.equal(error.code, 'BACKEND_UNAVAILABLE');
    assert.match(error.message, /@anthropic-ai\/claude-agent-sdk/);
    assert.match(error.message, /MODULE_NOT_FOUND/);
    assert.match(error.message, /plugin tree/);
    assert.match(error.message, /profile node_modules/);
    return true;
  });
});

test('claude SDK resolves from the profile package tree when the plugin tree lacks it', async t => {
  const workdir = await directory(t);
  const packageDir = join(workdir, 'node_modules', '@anthropic-ai', 'claude-agent-sdk');
  await mkdir(packageDir, { recursive: true });
  await writeFile(join(workdir, 'package.json'), JSON.stringify({ name: 'profile-fixture', dependencies: {} }));
  await writeFile(join(packageDir, 'package.json'), JSON.stringify({ name: '@anthropic-ai/claude-agent-sdk', version: '0.0.0', main: 'sdk.mjs' }));
  await writeFile(join(packageDir, 'sdk.mjs'), 'export const query = () => { throw new Error("not called"); };\nexport const marker = "fixture-sdk";\n');
  const isolated = join(workdir, 'isolated');
  await mkdir(isolated, { recursive: true });
  const resolved = await resolveAgentSdk({ profileDir: workdir, requireBase: join(isolated, 'plugin.js') });
  assert.equal(resolved.module.marker, 'fixture-sdk');
  assert.equal(typeof resolved.module.query, 'function');
  assert.match(resolved.resolvedFrom, /claude-agent-sdk[\\/]sdk\.mjs$/);
});

test('hat backends resolve by id, then display name, and default to dsh', () => {
  const run = { hats: [
    { id: 'planner', name: 'Planner', backend: 'claude-code', model: 'claude-opus-5-5' },
    { id: 'reviewer', name: 'Reviewer', backend: 'codex', model: 'gpt-6.1-sol' },
    { id: 'builder', name: 'Builder' },
  ] };
  assert.deepEqual(hatBackend(run, 'reviewer'), { type: 'codex', model: 'gpt-6.1-sol', hatId: 'reviewer' });
  assert.deepEqual(hatBackend(run, 'Planner'), { type: 'claude-code', model: 'claude-opus-5-5', hatId: 'planner' });
  assert.deepEqual(hatBackend(run, 'builder'), { type: 'dsh', model: undefined, hatId: 'builder' });
  assert.deepEqual(hatBackend({}, 'anything'), { type: 'dsh', model: undefined, hatId: 'anything' });
});

test('per-hat backend values accept only the DSH backend registry', () => {
  assert.deepEqual(HAT_BACKENDS, ['dsh', 'codex', 'claude-code']);
  assert.equal(parseHatBackend(undefined), 'dsh');
  assert.equal(parseHatBackend('dsh'), 'dsh');
  assert.equal(parseHatBackend('codex'), 'codex');
  assert.equal(parseHatBackend('claude-code'), 'claude-code');
  assert.throws(() => parseHatBackend('claude'), /must be one of dsh, codex, claude-code/);
  assert.throws(() => parseHatBackend({ type: 'codex' }), /must be one of dsh, codex, claude-code/);
  assert.throws(() => parseHatBackend({ command: 'x' }), /must be one of dsh, codex, claude-code/);
});

test('dsh per-hat models parse as provider/model selectors and reject malformed values', () => {
  assert.deepEqual(parseAgentModel('sparks/GLM-5.3-Flash-EXL3'), { provider: 'sparks', model: 'GLM-5.3-Flash-EXL3' });
  assert.equal(parseAgentModel(undefined), undefined);
  assert.throws(() => parseAgentModel('GLM-5.3-Flash-EXL3'), /provider\/model/);
  assert.throws(() => parseAgentModel('sparks/'), /provider\/model/);
  assert.throws(() => parseAgentModel('/GLM'), /provider\/model/);
  assert.throws(() => parseAgentModel('a/b/c'), /provider\/model/);
});

test('redaction masks credential-shaped values and known secret environment values', () => {
  const secrets = collectSecrets({ OPENAI_API_KEY: 'sk-livesecret123456', PATH: '/usr/bin', SHORT_TOKEN: 'abc' });
  assert.deepEqual(secrets, ['sk-livesecret123456']);
  const redacted = redact('key=sk-livesecret123456 Authorization: Bearer abcdefghijklmnop', secrets);
  assert.equal(redacted.includes('sk-livesecret123456'), false);
  assert.equal(redacted.includes('abcdefghijklmnop'), false);
  assert.match(redacted, /REDACTED/);
});

test('codex executable resolves config path, then PATH, then the profile codex package', async t => {
  const workdir = await directory(t);
  const configured = await resolveCodexExecutable({ codexPath: '/opt/codex/bin/codex', profileDir: workdir, env: { PATH: '' } });
  assert.deepEqual(configured, { command: '/opt/codex/bin/codex', prefixArgs: [] });
  const onPath = join(workdir, 'bin', 'codex');
  await mkdir(join(workdir, 'bin'), { recursive: true });
  await writeFile(onPath, '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  const found = await resolveCodexExecutable({ codexPath: '', profileDir: undefined, env: { PATH: join(workdir, 'bin') } });
  assert.deepEqual(found, { command: onPath, prefixArgs: [] });
});

test('codex executable falls back to the profile codex package and reports a precise failure otherwise', async t => {
  const profile = await directory(t);
  const packageDir = join(profile, 'node_modules', '@openai', 'codex');
  await mkdir(join(packageDir, 'bin'), { recursive: true });
  await writeFile(join(profile, 'package.json'), JSON.stringify({ name: 'profile-fixture' }));
  await writeFile(join(packageDir, 'package.json'), JSON.stringify({ name: '@openai/codex', version: '0.0.0', bin: { codex: 'bin/codex.js' } }));
  await writeFile(join(packageDir, 'bin', 'codex.js'), 'process.exit(0);\n');
  const fromProfile = await resolveCodexExecutable({ codexPath: '', profileDir: profile, env: { PATH: '' } });
  assert.equal(fromProfile.command, process.execPath);
  assert.match(fromProfile.prefixArgs.at(-1), /@openai[\\/]codex[\\/]bin[\\/]codex\.js$/);
  await assert.rejects(resolveCodexExecutable({ codexPath: '', profileDir: undefined, env: { PATH: '/nonexistent' } }), error => {
    assert.equal(error.code, 'CODEX_UNAVAILABLE');
    assert.match(error.message, /codexPath/);
    assert.match(error.message, /PATH/);
    assert.match(error.message, /@openai\/codex/);
    return true;
  });
});

test('backend failures always carry a stable machine code', () => {
  const error = backendError('TIMEOUT', 'claude iteration exceeded its session time limit');
  assert.equal(error.code, 'TIMEOUT');
  assert.equal(error.message, 'claude iteration exceeded its session time limit');
});
