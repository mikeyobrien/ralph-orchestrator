// Per-hat execution backends for the DSH Ralph orchestrator.
//
// `dsh` is the existing in-process bridge. `codex` drives the native
// `codex app-server --stdio` JSON-RPC protocol directly and `claude-code` drives
// the official Claude Agent SDK query surface. Neither native backend wraps the
// DSH subagent providers: every iteration owns its own child/query, its own
// timeout, and its own captured log under the run worktree.
import { spawn } from 'node:child_process';
import { constants as fsConstants } from 'node:fs';
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { createInterface } from 'node:readline';
import { delimiter, dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export const HAT_BACKENDS = ['dsh', 'codex', 'claude-code'];
export const CLAUDE_SDK_PACKAGE = '@anthropic-ai/claude-agent-sdk';
export const CODEX_PACKAGE = '@openai/codex';
const CLAUDE_SDK_VERSION = '0.3.263';
const DEFAULT_BACKEND = 'dsh';
const SECRET_NAME = /(?:^|_)(?:KEY|TOKEN|SECRET|PASSWORD|PASSWD|CREDENTIAL|CREDENTIALS)(?:$|_)/i;
const SECRET_PATTERNS = [
  /sk-[A-Za-z0-9_-]{8,}/g,
  /(Bearer\s+)[A-Za-z0-9._~+/-]{8,}/gi,
  /(Basic\s+)[A-Za-z0-9+/=]{8,}/gi,
];
const LOG_LIMIT = 64000;

export function backendError(code, message, cause) {
  return Object.assign(new Error(message), { code, ...(cause ? { cause } : {}) });
}

/** Credential-shaped values currently in the environment; never logged or returned. */
export function collectSecrets(env = process.env) {
  return Object.entries(env ?? {})
    .filter(([name, value]) => SECRET_NAME.test(name) && typeof value === 'string' && value.trim().length >= 8)
    .map(([, value]) => value.trim());
}

export function redact(text, secrets = []) {
  let value = String(text ?? '');
  for (const secret of secrets) {
    if (typeof secret !== 'string' || secret.length < 8) continue;
    value = value.split(secret).join('[REDACTED]');
  }
  value = value.replace(SECRET_PATTERNS[0], 'sk-[REDACTED]');
  value = value.replace(SECRET_PATTERNS[1], '$1[REDACTED]');
  value = value.replace(SECRET_PATTERNS[2], '$1[REDACTED]');
  return value;
}

/** Validate a hat's backend selector. Native Ralph backend names are not accepted. */
export function parseHatBackend(value) {
  if (value === undefined || value === null || value === '') return DEFAULT_BACKEND;
  if (typeof value === 'string' && HAT_BACKENDS.includes(value.trim())) return value.trim();
  throw backendError('INVALID_BACKEND', `hat backend must be one of ${HAT_BACKENDS.join(', ')}; received ${JSON.stringify(value)}. Native Ralph backend names and custom commands cannot bypass the DSH plugin.`);
}

/** Parse a `provider/model` selector for the in-process DSH backend. */
export function parseAgentModel(value) {
  if (value === undefined || value === null || value === '') return undefined;
  const text = typeof value === 'string' ? value.trim() : '';
  const match = /^([^\s/]+)\/([^\s/]+)$/.exec(text);
  if (!match) throw backendError('INVALID_MODEL', 'per-hat dsh model must use the provider/model form (for example sparks/GLM-5.3-Flash-EXL3)');
  return { provider: match[1], model: match[2] };
}

/** The effective execution backend of one hat in one run, defaulting to the DSH bridge. */
export function hatBackend(run, hatId) {
  const hats = Array.isArray(run?.hats) ? run.hats : [];
  const matched = hats.find(hat => hat.id === hatId) ?? hats.find(hat => hat.name === hatId);
  const type = parseHatBackend(matched?.backend);
  const model = matched?.model === undefined || matched?.model === null || matched?.model === '' ? undefined : String(matched.model);
  return { type, model, hatId: matched?.id ?? hatId };
}

async function executableInPath(name, env) {
  for (const directory of String(env.PATH ?? '').split(delimiter).filter(Boolean)) {
    const candidate = join(directory, name);
    try { await access(candidate, fsConstants.X_OK); return candidate; } catch {}
  }
  return undefined;
}

/**
 * Resolve the native Codex CLI without copying or wrapping it: an explicit
 * path, then PATH, then the profile's installed @openai/codex package.
 */
export async function resolveCodexExecutable({ codexPath = '', profileDir, env = process.env } = {}) {
  const attempts = [];
  const configured = String(codexPath ?? '').trim();
  if (configured) return { command: configured, prefixArgs: [] };
  attempts.push('codexPath: not configured');
  try {
    const found = await executableInPath('codex', env);
    if (found) return { command: found, prefixArgs: [] };
    attempts.push(`PATH: no executable "codex" in ${String(env.PATH ?? '') || '(empty)'}`);
  } catch (error) { attempts.push(`PATH: ${error.message}`); }
  if (profileDir) {
    try {
      const manifestPath = createRequire(join(profileDir, 'package.json')).resolve(`${CODEX_PACKAGE}/package.json`);
      const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
      const bin = typeof manifest.bin === 'string' ? manifest.bin : manifest.bin?.codex;
      if (typeof bin === 'string' && bin.length > 0) {
        const binPath = resolve(dirname(manifestPath), bin);
        return { command: process.execPath, prefixArgs: [binPath] };
      }
      attempts.push(`${CODEX_PACKAGE}: manifest has no codex bin`);
    } catch (error) { attempts.push(`${CODEX_PACKAGE}: ${error.code || error.name}: ${String(error.message).split('\n')[0]}`); }
  } else attempts.push(`${CODEX_PACKAGE}: no DSH profile directory is available`);
  throw backendError('CODEX_UNAVAILABLE', `codex backend unavailable: no Codex CLI was found. Tried -> ${attempts.join(' | ')}`);
}

/**
 * Resolve the official Claude Agent SDK from an already-installed tree. The
 * plugin never vendors or copies it: the plugin tree is tried first, then the
 * active DSH profile's node_modules. Both failures are reported verbatim.
 */
export async function resolveAgentSdk({ profileDir, requireBase = import.meta.url } = {}) {
  const attempts = [];
  const bases = [['plugin tree', requireBase]];
  if (profileDir) bases.push([`profile node_modules (${profileDir})`, join(profileDir, 'package.json')]);
  else attempts.push('profile node_modules: no DSH profile directory is available');
  for (const [label, base] of bases) {
    try {
      const resolvedFrom = createRequire(base).resolve(CLAUDE_SDK_PACKAGE);
      const module = await import(pathToFileURL(resolvedFrom).href);
      return { module, resolvedFrom, resolvedVia: label };
    } catch (error) {
      attempts.push(`${label}: ${error.code || error.name}: ${String(error.message).split('\n')[0]}`);
    }
  }
  throw backendError('BACKEND_UNAVAILABLE', `claude-code backend unavailable: cannot resolve ${CLAUDE_SDK_PACKAGE}@${CLAUDE_SDK_VERSION} (no vendored copy is used). Tried -> ${attempts.join(' | ')}`);
}

function segment(value) { return String(value ?? '').replace(/[\x00-\x1f\x7f]+/g, ' ').trim(); }

async function writeBackendLog({ logPath, backend, model, cwd, argv, prompt, stdout, stderr, outcome, started, durationMs, secrets }) {
  if (!logPath) return;
  const body = [
    `# backend: ${backend}`,
    `# model: ${model ?? '(default)'}`,
    `# cwd: ${segment(cwd)}`,
    `# argv: ${segment(argv)}`,
    `# started: ${new Date(started).toISOString()}`,
    `# durationMs: ${durationMs}`,
    `# outcome: ${segment(outcome)}`,
    '',
    '## prompt',
    prompt,
    '',
    '## stdout',
    stdout,
    '',
    '## stderr',
    stderr,
    '',
  ].join('\n');
  try {
    await mkdir(dirname(logPath), { recursive: true });
    await writeFile(logPath, redact(body, secrets), { mode: 0o600 });
  } catch {
    // Log capture must never replace the iteration's real outcome.
  }
}

function terminateChild(child) {
  if (child.exitCode !== null || child.signalCode) return;
  const signalChild = signal => {
    try { process.kill(process.platform === 'win32' ? child.pid : -child.pid, signal); }
    catch { try { child.kill(signal); } catch {} }
  };
  signalChild('SIGTERM');
  const hard = setTimeout(() => signalChild('SIGKILL'), 500);
  hard.unref?.();
}

/**
 * One codex iteration through `codex app-server --stdio`: initialize handshake,
 * one ephemeral thread, one text turn, terminal-turn result, guaranteed child
 * teardown, timeout/abandon kill, and a redacted log beside the run.
 */
export async function runCodexIteration({ command, prefixArgs = [], cwd, prompt, model, env = process.env, signal, timeoutMs = 600000, logPath }) {
  if (typeof prompt !== 'string' || prompt.trim().length === 0) throw backendError('INVALID_PROMPT', 'codex iteration requires a non-empty prompt');
  if (signal?.aborted) throw backendError('ABORTED', 'codex iteration was abandoned');
  if (typeof command !== 'string' || command.length === 0) throw backendError('CODEX_UNAVAILABLE', 'codex backend unavailable: no executable was resolved');
  const started = Date.now();
  const secrets = collectSecrets(env);
  const argv = [...prefixArgs, 'app-server', '--stdio'];
  let stdout = '', stderr = '';
  let outcome = 'starting';
  let exitCode;
  const child = spawn(command, argv, { cwd, env, stdio: ['pipe', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
  child.stdin.on('error', () => {});
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-LOG_LIMIT); });
  let settleExit;
  const exited = new Promise(done => { settleExit = done; });
  child.once('error', error => settleExit({ code: null, signalCode: null, error }));
  child.once('close', (code, signalCode) => settleExit({ code, signalCode }));
  const pending = new Map();
  let nextId = 0;
  const write = frame => { if (child.stdin.writable) child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', ...frame })}\n`); };
  const request = (method, params) => new Promise((resolveRequest, rejectRequest) => {
    const id = `r${++nextId}`;
    pending.set(id, { resolve: resolveRequest, reject: rejectRequest });
    write({ id, method, params });
  });
  const failAll = error => { for (const entry of pending.values()) entry.reject(error); pending.clear(); };
  const state = { threadId: undefined, turnId: undefined, pendingTurnId: undefined, early: [] };
  const answers = { final: undefined, partial: undefined, unphased: undefined };
  let resolveTerminal, rejectTerminal;
  const terminal = new Promise((resolvePromise, rejectPromise) => { resolveTerminal = resolvePromise; rejectTerminal = rejectPromise; });
  terminal.catch(() => {});
  let stopped;
  const stop = reason => {
    if (stopped) return;
    stopped = reason;
    outcome = reason;
    terminateChild(child);
    const message = reason === 'timeout' ? `codex iteration exceeded its session time limit (${timeoutMs}ms)` : 'codex iteration was abandoned';
    rejectTerminal(backendError(reason === 'timeout' ? 'TIMEOUT' : 'ABORTED', message));
  };
  const timer = timeoutMs > 0 ? setTimeout(() => stop('timeout'), timeoutMs) : undefined;
  timer?.unref?.();
  const onAbort = () => stop('abort');
  signal?.addEventListener('abort', onAbort, { once: true });
  if (signal?.aborted) stop('abort');

  const observeTurnId = id => {
    if (typeof id !== 'string' || id.length === 0) throw backendError('PROTOCOL', 'codex app-server referenced an invalid turn');
    if (state.pendingTurnId !== undefined && state.pendingTurnId !== id) throw backendError('PROTOCOL', 'codex app-server referenced conflicting turns');
    state.pendingTurnId = id;
  };
  const commitTurnId = id => {
    observeTurnId(id);
    state.turnId = id;
    for (const notification of state.early.splice(0)) handleNotification(notification.method, notification.params);
  };
  function handleNotification(method, params = {}) {
    if (method === 'turn/started') {
      if (params.threadId !== state.threadId) return;
      if (state.turnId === undefined) observeTurnId(params.turn?.id);
      return;
    }
    if (method === 'item/completed') {
      if (params.threadId !== state.threadId) return;
      if (state.turnId === undefined) { observeTurnId(params.turnId); state.early.push({ method, params }); return; }
      if (params.turnId !== state.turnId) return;
      const item = params.item;
      if (item?.type !== 'agentMessage' || typeof item.text !== 'string') return;
      if (item.phase === 'final_answer') answers.final = item.text;
      else if (item.phase === 'partial_answer') answers.partial = item.text;
      else if (item.phase === null) answers.unphased = item.text;
      return;
    }
    if (method !== 'turn/completed') return;
    if (params.threadId !== state.threadId) return;
    const turn = params.turn;
    if (state.turnId === undefined) { observeTurnId(turn?.id); state.early.push({ method, params }); return; }
    if (turn?.id !== state.turnId) return;
    resolveTerminal(turn);
  }
  function handleServerRequest(frame) {
    let result;
    switch (frame.method) {
      case 'item/commandExecution/requestApproval':
      case 'item/fileChange/requestApproval': {
        const decision = Array.isArray(frame.params?.availableDecisions) ? frame.params.availableDecisions : [];
        result = { decision: decision.includes('decline') ? 'decline' : decision.includes('cancel') ? 'cancel' : 'decline' };
        break;
      }
      case 'item/permissions/requestApproval': result = { permissions: {}, scope: 'turn' }; break;
      case 'item/tool/requestUserInput': result = { answers: {} }; break;
      case 'mcpServer/elicitation/request': result = { action: 'decline', content: null, _meta: null }; break;
      default:
        rejectTerminal(backendError('PROTOCOL', `codex app-server requested an unsupported interaction: ${frame.method}`));
        return;
    }
    write({ id: frame.id, result });
  }
  const lines = createInterface({ input: child.stdout });
  lines.on('line', line => {
    stdout = (stdout + line + '\n').slice(-LOG_LIMIT);
    let frame;
    try { frame = JSON.parse(line); } catch { return; }
    if (frame === null || typeof frame !== 'object') return;
    if (frame.method === undefined) {
      const entry = pending.get(frame.id);
      if (!entry) return;
      pending.delete(frame.id);
      if (frame.error !== undefined) entry.reject(backendError('PROTOCOL', `codex app-server rejected a request: ${frame.error?.message ?? JSON.stringify(frame.error)}`));
      else entry.resolve(frame.result);
      return;
    }
    if (frame.id !== undefined) { handleServerRequest(frame); return; }
    try { handleNotification(frame.method, frame.params ?? {}); }
    catch (error) { rejectTerminal(error); }
  });
  exited.then(result => {
    failAll(backendError('PROTOCOL', `codex app-server closed before replying: ${result.error?.message ?? 'protocol stream closed'}`));
    rejectTerminal(backendError('PROTOCOL', `codex app-server exited before the turn completed (code ${result.code ?? 'spawn-error'}${result.signalCode ? `, signal ${result.signalCode}` : ''}): ${redact(segment(stderr), secrets) || result.error?.message || 'no stderr'}`));
  });

  let output;
  try {
    const handshake = await request('initialize', { clientInfo: { name: 'dsh-ralph-orchestrator', title: 'DSH Ralph Orchestrator', version: '0.1.0' }, capabilities: { experimentalApi: false, requestAttestation: false } });
    const userAgent = typeof handshake?.userAgent === 'string' ? handshake.userAgent : 'unknown';
    write({ method: 'initialized' });
    // Fail before any work when the resolved CLI does not offer the selected model.
    if (model) {
      try {
        const offered = [];
        let cursor;
        for (let page = 0; page < 5; page += 1) {
          const catalog = await request('model/list', { includeHidden: true, ...(cursor ? { cursor } : {}) });
          for (const item of Array.isArray(catalog?.data) ? catalog.data : []) if (typeof item?.id === 'string') offered.push(item.id);
          cursor = typeof catalog?.nextCursor === 'string' && catalog.nextCursor.length > 0 ? catalog.nextCursor : undefined;
          if (!cursor) break;
        }
        if (offered.length > 0 && !offered.includes(model)) throw backendError('MODEL_UNAVAILABLE', `codex app-server (${userAgent}) does not offer model ${model}; available: ${offered.join(', ')}`);
      } catch (error) {
        if (error?.code === 'MODEL_UNAVAILABLE') throw error;
        // A server that cannot list models still gets the requested model on thread/start.
      }
    }
    const thread = (await request('thread/start', { cwd, ephemeral: true, ...(model ? { model } : {}), approvalPolicy: 'never' }))?.thread;
    if (typeof thread?.id !== 'string' || thread.id.length === 0) throw backendError('PROTOCOL', 'codex app-server returned an invalid thread');
    if (thread.ephemeral !== true) throw backendError('PROTOCOL', 'codex app-server did not create an ephemeral thread');
    state.threadId = thread.id;
    const turn = (await request('turn/start', { threadId: thread.id, input: [{ type: 'text', text: prompt, text_elements: [] }] }))?.turn;
    if (typeof turn?.id !== 'string' || turn.id.length === 0) throw backendError('PROTOCOL', 'codex app-server returned an invalid turn');
    commitTurnId(turn.id);
    const terminalTurn = await terminal;
    if (terminalTurn.status !== 'completed') {
      const detail = terminalTurn.error?.codexErrorInfo ? `: ${terminalTurn.error.codexErrorInfo}` : '';
      throw backendError('TURN_FAILED', `codex turn ended with status ${String(terminalTurn.status)}${detail}`);
    }
    output = answers.final ?? answers.partial ?? answers.unphased;
    if (typeof output !== 'string' || output.trim().length === 0) throw backendError('TURN_FAILED', 'codex completed without an answer');
    outcome = 'completed';
    // Close the protocol stdin first so a well-behaved server exits cleanly;
    // only a server that ignores EOF is terminated.
    try { child.stdin.end(); } catch {}
    const result = await Promise.race([exited, new Promise(done => setTimeout(() => done({ code: null, signalCode: null, timedOut: true }), 1500))]);
    if (result.timedOut) terminateChild(child);
    exitCode = result.code ?? undefined;
    return { output, exitCode, stderrTail: redact(segment(stderr).slice(-4000), secrets), durationMs: Date.now() - started, logPath };
  } catch (error) {
    if (stopped === 'timeout') throw backendError('TIMEOUT', `codex iteration exceeded its session time limit (${timeoutMs}ms)`, error);
    if (stopped === 'abort') throw backendError('ABORTED', 'codex iteration was abandoned', error);
    if (error?.code) throw error;
    if (signal?.aborted) throw backendError('ABORTED', 'codex iteration was abandoned', error);
    throw backendError('CODEX_FAILED', `codex iteration failed: ${error?.message ?? String(error)}`, error);
  } finally {
    if (timer) clearTimeout(timer);
    signal?.removeEventListener('abort', onAbort);
    terminateChild(child);
    const settled = await Promise.race([exited, new Promise(done => setTimeout(() => done({ code: null, signalCode: null, timedOut: true }), 2000))]);
    exitCode ??= settled.code ?? undefined;
    lines.close();
    if (outcome !== 'completed' && stopped === undefined) outcome = 'failed';
    await writeBackendLog({ logPath, backend: 'codex', model, cwd, argv: `${command} ${argv.join(' ')}`, prompt, stdout, stderr, outcome: `${outcome}${exitCode === undefined ? '' : ` (exit ${exitCode})`}`, started, durationMs: Date.now() - started, secrets });
  }
}

function sdkResult(message) {
  if (message.subtype !== 'success') {
    const detail = Array.isArray(message.errors) && message.errors.length ? `: ${message.errors.join('; ')}` : '';
    throw backendError('SDK_FAILURE', `claude-code SDK result ${String(message.subtype)}${detail}`);
  }
  if (message.is_error || typeof message.result !== 'string' || message.result.trim().length === 0) throw backendError('SDK_FAILURE', 'claude-code SDK returned an invalid success result');
  return message.result;
}

/**
 * One claude-code iteration through the official Agent SDK: a fresh query per
 * iteration, the exact Ralph-assembled prompt, unattended permissions inside
 * the isolated worktree, timeout/abandon cancellation, and a redacted log.
 */
export async function runClaudeIteration({ sdk, cwd, prompt, model, env, signal, timeoutMs = 600000, logPath }) {
  if (!sdk || typeof sdk.query !== 'function') throw backendError('BACKEND_UNAVAILABLE', 'claude-code backend unavailable: the resolved Agent SDK exposes no query() function');
  if (typeof prompt !== 'string' || prompt.trim().length === 0) throw backendError('INVALID_PROMPT', 'claude-code iteration requires a non-empty prompt');
  if (signal?.aborted) throw backendError('ABORTED', 'claude-code iteration was abandoned');
  const started = Date.now();
  const secrets = collectSecrets(env ?? process.env);
  const controller = new AbortController();
  let stopped;
  const stop = reason => {
    if (stopped) return;
    stopped = reason;
    controller.abort(backendError(reason === 'timeout' ? 'TIMEOUT' : 'ABORTED', reason === 'timeout' ? 'claude-code iteration exceeded its session time limit' : 'claude-code iteration was abandoned'));
  };
  const timer = timeoutMs > 0 ? setTimeout(() => stop('timeout'), timeoutMs) : undefined;
  timer?.unref?.();
  const onAbort = () => stop('abort');
  signal?.addEventListener('abort', onAbort, { once: true });
  if (signal?.aborted) stop('abort');
  let stream;
  let outcome = stopped ?? 'starting';
  let output;
  try {
    stream = sdk.query({
      prompt,
      options: {
        abortController: controller,
        cwd,
        ...(model ? { model } : {}),
        ...(env ? { env } : {}),
        persistSession: false,
        permissionMode: 'bypassPermissions',
        allowDangerouslySkipPermissions: true,
      },
    });
    for await (const message of stream) {
      if (stopped) throw backendError(stopped === 'timeout' ? 'TIMEOUT' : 'ABORTED', stopped === 'timeout' ? `claude-code iteration exceeded its session time limit (${timeoutMs}ms)` : 'claude-code iteration was abandoned');
      if (message?.type !== 'result') continue;
      output = sdkResult(message);
    }
    if (output === undefined) throw backendError('SDK_FAILURE', 'claude-code finished without a result message');
    outcome = 'completed';
    return { output, exitCode: undefined, stderrTail: '', durationMs: Date.now() - started, logPath };
  } catch (error) {
    if (stopped === 'timeout') throw backendError('TIMEOUT', `claude-code iteration exceeded its session time limit (${timeoutMs}ms)`, error);
    if (stopped === 'abort' || signal?.aborted) throw backendError('ABORTED', 'claude-code iteration was abandoned', error);
    if (error?.code) throw error;
    throw backendError('SDK_FAILURE', `claude-code SDK failed: ${error?.message ?? String(error)}`, error);
  } finally {
    if (timer) clearTimeout(timer);
    signal?.removeEventListener('abort', onAbort);
    if (!controller.signal.aborted) controller.abort(backendError('ABORTED', 'iteration finished'));
    try { stream?.close?.(); } catch {}
    if (outcome !== 'completed') outcome = stopped ?? 'failed';
    await writeBackendLog({ logPath, backend: 'claude-code', model, cwd, argv: `Agent SDK query() ${model ? `model=${model}` : ''}`, prompt, stdout: output ?? '', stderr: '', outcome, started, durationMs: Date.now() - started, secrets });
  }
}
