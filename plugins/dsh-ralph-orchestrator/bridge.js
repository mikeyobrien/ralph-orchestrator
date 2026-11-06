#!/usr/bin/env node
import { connect } from 'node:net';
import { readFile, stat } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';

async function main() {
  const args = process.argv.slice(2);
  if (args.length !== 2 || args[0] !== '--prompt-file') throw new Error('The DSH bridge requires explicit --prompt-file transport');
  const info = await stat(args[1]);
  if (info.size > 2 * 1024 * 1024) throw new Error('Ralph prompt exceeds the 2 MiB bridge limit');
  const prompt = await readFile(args[1], 'utf8');
  const socketPath = process.env.DSH_RALPH_BRIDGE_SOCKET;
  const token = process.env.DSH_RALPH_BRIDGE_TOKEN;
  const runId = process.env.DSH_RALPH_RUN_ID;
  if (!socketPath || !token || !runId) throw new Error('No owning DSH Ralph run is available');
  const env = Object.fromEntries(['RALPH_BIN', 'RALPH_WORKSPACE_ROOT', 'RALPH_EVENTS_FILE', 'PATH'].filter(key => process.env[key] !== undefined).map(key => [key, process.env[key]]));
  if (process.env.RALPH_WAVE_WORKER) throw new Error('DSH Ralph does not support wave workers yet');
  const response = await new Promise((resolve, reject) => {
    const socket = connect(socketPath);
    socket.setEncoding('utf8');
    let received = '', done = false;
    socket.on('connect', () => socket.write(JSON.stringify({ id: randomUUID(), runId, token, prompt, cwd: process.cwd(), env, pid: process.pid }) + '\n'));
    socket.on('error', reject);
    socket.on('data', chunk => {
      received += chunk.toString('utf8');
      if (received.length > 4 * 1024 * 1024) { socket.destroy(); reject(new Error('DSH response exceeds bridge limit')); return; }
      const end = received.indexOf('\n');
      if (end !== -1) {
        try { const result = JSON.parse(received.slice(0, end)); done = true; socket.end(); resolve(result); }
        catch (error) { socket.destroy(); reject(error); }
      }
    });
    socket.on('close', () => { if (!done) reject(new Error('The owning DSH run disconnected')); });
  });
  if (!response.ok) throw new Error(response.error || 'DSH iteration failed');
  process.stdout.write(response.output || '');
}
main().catch(error => { process.stderr.write(`DSH Ralph: ${error.message}\n`); process.exitCode = 1; });
