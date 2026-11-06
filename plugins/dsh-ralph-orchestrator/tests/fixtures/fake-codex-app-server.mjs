#!/usr/bin/env node
// Scripted stand-in for `codex app-server --stdio`.
//
// Scenarios (FAKE_CODEX_SCENARIO):
//   ok       initialize/thread/turn handshake, one final answer, completed turn
//   partial  a partial_answer item followed by the final_answer item
//   noise    a non-JSON stdout diagnostic before the terminal notification
//   approval a server-initiated command approval request, then completion
//   error    turn/completed with status failed
//   exit     protocol child dies mid-turn with a non-zero exit
//   hang     handshake succeeds, the turn never terminates
//
// FAKE_CODEX_TRACE appends one JSON line per observed fact so assertions can
// inspect argv, cwd, thread and turn mapping, and approval decisions.
import { appendFileSync } from 'node:fs';
import { createInterface } from 'node:readline';

const tracePath = process.env.FAKE_CODEX_TRACE;
const scenario = process.env.FAKE_CODEX_SCENARIO || 'ok';
const trace = entry => { if (tracePath) appendFileSync(tracePath, `${JSON.stringify(entry)}\n`); };
const argv = process.argv.slice(2);
trace({ kind: 'spawn', argv, cwd: process.cwd(), pid: process.pid, scenario, env: { RALPH_BIN: process.env.RALPH_BIN ?? null, RALPH_WORKSPACE_ROOT: process.env.RALPH_WORKSPACE_ROOT ?? null, RALPH_EVENTS_FILE: process.env.RALPH_EVENTS_FILE ?? null } });
if (JSON.stringify(argv) !== JSON.stringify(['app-server', '--stdio'])) {
  process.stderr.write(`fake-codex: unexpected argv ${JSON.stringify(argv)}\n`);
  process.exit(64);
}
const threadId = 'th_fake';
const turnId = 'tn_fake';
const send = frame => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', ...frame })}\n`);
const reply = (id, result) => send({ id, result });
const finish = () => setTimeout(() => process.exit(0), 30);
let awaitingApproval = false;

function completeTurn(answer, extra = []) {
  for (const text of extra) send({ method: 'item/completed', params: { threadId, turnId, item: { type: 'agentMessage', id: `m-${text.length}`, text, phase: 'partial_answer' } } });
  send({ method: 'item/completed', params: { threadId, turnId, item: { type: 'agentMessage', id: 'm-final', text: answer, phase: 'final_answer' } } });
  send({ method: 'turn/completed', params: { threadId, turn: { id: turnId, status: 'completed' } } });
  finish();
}

createInterface({ input: process.stdin }).on('line', line => {
  let frame;
  try { frame = JSON.parse(line); } catch { trace({ kind: 'invalid', line }); return; }
  const isResponse = frame.id !== undefined && frame.method === undefined;
  if (isResponse) {
    trace({ kind: 'response', id: frame.id, result: frame.result ?? null, error: frame.error ?? null });
    if (awaitingApproval) { awaitingApproval = false; completeTurn('approved request handled'); }
    return;
  }
  trace({ kind: 'request', method: frame.method, params: frame.params });
  switch (frame.method) {
    case 'initialize':
      return reply(frame.id, { userAgent: 'fake-codex/1.0.0' });
    case 'initialized':
      return;
    case 'model/list': {
      const data = scenario === 'catalog-missing' ? [{ id: 'gpt-5.6-sol' }, { id: 'gpt-6-astra' }] : [{ id: 'gpt-6.1-sol' }, { id: 'gpt-6.1-terra' }, { id: 'gpt-6-astra' }];
      trace({ kind: 'catalog', includeHidden: frame.params?.includeHidden ?? null, ids: data.map(item => item.id) });
      return reply(frame.id, { data, nextCursor: null });
    }
    case 'thread/start':
      trace({ kind: 'thread', cwd: frame.params.cwd, model: frame.params.model ?? null, ephemeral: frame.params.ephemeral, approvalPolicy: frame.params.approvalPolicy ?? null });
      return reply(frame.id, { thread: { id: threadId, ephemeral: true } });
    case 'turn/start':
      trace({ kind: 'turn', threadId: frame.params.threadId, input: frame.params.input });
      reply(frame.id, { turn: { id: turnId } });
      send({ method: 'turn/started', params: { threadId, turn: { id: turnId, status: 'inProgress' } } });
      if (scenario === 'hang') return;
      if (scenario === 'noise') process.stdout.write('fake-codex server diagnostic that is not a frame\n');
      if (scenario === 'exit') { process.stderr.write('fake-codex exploded mid-turn\n'); setTimeout(() => process.exit(3), 20); return; }
      if (scenario === 'stderr') { process.stderr.write(`fake-codex warning OPENAI_API_KEY=${process.env.OPENAI_API_KEY ?? ''} rejected\n`); return completeTurn('fake codex final answer'); }
      if (scenario === 'error') return send({ method: 'turn/completed', params: { threadId, turn: { id: turnId, status: 'failed', error: { codexErrorInfo: 'serverOverloaded' } } } }), finish();
      if (scenario === 'approval') {
        awaitingApproval = true;
        const availableDecisions = ['accept', 'decline', 'cancel'];
        send({ id: 'srv-1', method: 'item/commandExecution/requestApproval', params: { threadId, turnId, itemId: 'c1', availableDecisions } });
        return;
      }
      if (scenario === 'partial') return completeTurn('fake codex final answer', ['earlier partial draft']);
      return completeTurn('fake codex final answer');
    case 'turn/interrupt':
      return reply(frame.id, {});
    default:
      trace({ kind: 'unsupported', method: frame.method });
      if (frame.id !== undefined) send({ id: frame.id, error: { code: -32601, message: `unsupported ${frame.method}` } });
  }
});

setInterval(() => {}, 1000).unref();
