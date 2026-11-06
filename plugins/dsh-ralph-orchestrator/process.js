import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { randomUUID } from 'node:crypto';

export class RalphProcess {
  constructor({ executable, args, cwd, env, onEvent = () => {} }) {
    this.pending = new Map();
    this.stderr = '';
    this.terminal = undefined;
    this.child = spawn(executable, args, { cwd, env, detached: process.platform !== 'win32', stdio: ['pipe', 'pipe', 'pipe'] });
    this.child.stdin.on('error', () => {}); // Pending requests settle on the owned process's exit.
    this.child.stderr.on('data', chunk => { this.stderr = (this.stderr + chunk).slice(-12000); });
    const lines = createInterface({ input: this.child.stdout });
    lines.on('line', line => {
      let event;
      try { event = JSON.parse(line); } catch { this.stderr = (this.stderr + '\n' + line).slice(-12000); return; }
      if (event.type === 'response' && event.id && this.pending.has(event.id)) {
        const request = this.pending.get(event.id);
        this.pending.delete(event.id);
        clearTimeout(request.timer);
        event.success ? request.resolve(event.data) : request.reject(new Error(event.error || 'Ralph rejected the RPC request'));
      }
      if (event.type === 'loop_terminated') {
        this.terminal = event;
        // The protocol is terminal: release the command reader's EOF barrier,
        // but still observe real process close before reporting settlement.
        this.child.stdin.end();
      }
      // Observers must not stall the continuously drained protocol stream.
      try { onEvent(event); } catch (error) { this.stderr = (this.stderr + '\n' + error.message).slice(-12000); }
    });
    this.finished = new Promise(resolve => {
      let spawnError;
      this.child.once('error', error => { spawnError = error; });
      this.child.once('close', (code, signal) => {
        lines.close();
        for (const request of this.pending.values()) { clearTimeout(request.timer); request.reject(spawnError || new Error('Ralph exited before replying')); }
        this.pending.clear();
        resolve({ code, signal, terminal: this.terminal, stderr: this.stderr, error: spawnError?.message });
      });
    });
  }
  request(type, fields = {}, timeoutMs = 10000) {
    if (!this.child.stdin.writable) return Promise.reject(new Error('Ralph RPC is closed'));
    const id = randomUUID();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`Ralph ${type} timed out`)); }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.child.stdin.write(JSON.stringify({ type, id, ...fields }) + '\n');
    });
  }
  signal(signal) {
    if (this.child.exitCode !== null || this.child.signalCode) return;
    try { process.kill(process.platform === 'win32' ? this.child.pid : -this.child.pid, signal); }
    catch (error) { if (error.code !== 'ESRCH') throw error; }
  }
  async stop() {
    await this.request('abort', { reason: 'Stopped from DSH' }, 1000).catch(() => {});
    const settled = await Promise.race([this.finished.then(() => true), new Promise(resolve => { const timer = setTimeout(() => resolve(false), 1500); timer.unref(); })]);
    if (!settled) {
      this.signal('SIGTERM');
      const timer = setTimeout(() => this.signal('SIGKILL'), 1000);
      timer.unref();
      try { await this.finished; } finally { clearTimeout(timer); }
    }
    return this.finished;
  }
}

export function runCommand(executable, args, { cwd, env, signal, maxBytes = 32000 } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { cwd, env, signal, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '';
    child.stdout.on('data', chunk => { stdout = (stdout + chunk).slice(-maxBytes); });
    child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-maxBytes); });
    child.once('error', reject);
    child.once('close', (code, exitSignal) => {
      if (code === 0) resolve({ stdout, stderr });
      else reject(Object.assign(new Error(stderr.trim() || `${executable} exited with ${code ?? exitSignal}`), { code: 'PROCESS_FAILED', exitCode: code }));
    });
  });
}
