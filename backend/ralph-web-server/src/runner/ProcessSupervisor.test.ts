import { test } from "node:test";
import assert from "node:assert";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { ProcessSupervisor } from "./ProcessSupervisor";

const testRunDir = path.join(os.tmpdir(), "ralph-test-runs");

test("ProcessSupervisor.spawn creates task directory and PID file", () => {
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const taskId = "test-spawn-" + Date.now();

  const handle = supervisor.spawn(taskId, "test prompt", ["--version"], process.cwd());

  assert.ok(handle.pid > 0);
  assert.strictEqual(handle.taskId, taskId);
  assert.ok(fs.existsSync(handle.taskDir));
  assert.ok(fs.existsSync(path.join(handle.taskDir, "pid")));
  assert.ok(fs.existsSync(path.join(handle.taskDir, "status.json")));

  // Cleanup
  fs.rmSync(handle.taskDir, { recursive: true, force: true });
});

test("ProcessSupervisor.spawn writes initial status", () => {
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const taskId = "test-status-" + Date.now();

  const handle = supervisor.spawn(taskId, "test prompt", ["--version"], process.cwd());
  const status = supervisor.getStatus(taskId);

  assert.ok(status);
  assert.strictEqual(status.state, "running");
  assert.ok(status.startedAt);

  // Cleanup
  fs.rmSync(handle.taskDir, { recursive: true, force: true });
});

test("ProcessSupervisor.reconnect returns null for non-existent task", () => {
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const handle = supervisor.reconnect("non-existent-task");

  assert.strictEqual(handle, null);
});

test("ProcessSupervisor.reconnect returns handle for existing process", () => {
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const taskId = "test-reconnect-" + Date.now();

  const original = supervisor.spawn(taskId, "test prompt", ["--version"], process.cwd());
  const reconnected = supervisor.reconnect(taskId);

  assert.ok(reconnected);
  assert.strictEqual(reconnected.taskId, taskId);
  assert.strictEqual(reconnected.pid, original.pid);

  // Cleanup
  fs.rmSync(original.taskDir, { recursive: true, force: true });
});

test("ProcessSupervisor.isAlive returns true for current process", () => {
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const alive = supervisor.isAlive(process.pid);

  assert.strictEqual(alive, true);
});

test("ProcessSupervisor.isAlive returns false for non-existent PID", () => {
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const alive = supervisor.isAlive(999999);

  assert.strictEqual(alive, false);
});

test("ProcessSupervisor.getStatus returns null for non-existent task", () => {
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const status = supervisor.getStatus("non-existent-task");

  assert.strictEqual(status, null);
});

test("ProcessSupervisor.getStatus returns status for existing task", () => {
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const taskId = "test-getstatus-" + Date.now();

  const handle = supervisor.spawn(taskId, "test prompt", ["--version"], process.cwd());
  const status = supervisor.getStatus(taskId);

  assert.ok(status);
  assert.strictEqual(status.state, "running");

  // Cleanup
  fs.rmSync(handle.taskDir, { recursive: true, force: true });
});

test("ProcessSupervisor.spawn treats shell metacharacters as literals (CVE regression)", () => {
  // This test verifies the fix for command injection vulnerability.
  // With shell: true, these metacharacters would be interpreted as shell commands.
  // With array-form spawn, they are passed as literal arguments.
  const supervisor = new ProcessSupervisor({ runDir: testRunDir });
  const taskId = "test-injection-" + Date.now();

  // These would execute arbitrary code with shell: true
  const maliciousArgs = [
    "--version",
    "; echo INJECTED",      // Shell command separator
    "| cat /etc/passwd",    // Pipe to another command
    "$(whoami)",            // Command substitution
    "`id`",                 // Backtick command substitution
  ];

  const handle = supervisor.spawn(taskId, "test prompt", maliciousArgs, process.cwd());

  // If we got here without spawning additional processes, the injection was prevented
  assert.ok(handle.pid > 0);
  assert.strictEqual(handle.taskId, taskId);

  // Verify stdout.log exists (proves spawn worked)
  assert.ok(fs.existsSync(path.join(handle.taskDir, "stdout.log")));

  // The key verification: stderr should contain an error about unrecognized args,
  // NOT output from injected commands. With shell:true, echo INJECTED would write
  // "INJECTED" to stdout. With array-form spawn, ralph sees "; echo INJECTED" as
  // a literal invalid argument.

  // Wait briefly for process to start and produce output
  const maxWait = 500;
  const start = Date.now();
  while (Date.now() - start < maxWait) {
    // Poll for stderr content
    const stderrPath = path.join(handle.taskDir, "stderr.log");
    if (fs.existsSync(stderrPath)) {
      const stderr = fs.readFileSync(stderrPath, "utf-8");
      // If ralph received the malicious args as literal arguments, it will complain
      // about unrecognized options. This is the expected safe behavior.
      if (stderr.length > 0) {
        break;
      }
    }
  }

  // Verify that shell metacharacters were NOT interpreted
  const stdoutPath = path.join(handle.taskDir, "stdout.log");
  const stdout = fs.readFileSync(stdoutPath, "utf-8");

  // CRITICAL: With shell:true, "echo INJECTED" would write "INJECTED" to stdout
  // With array-form spawn, no shell commands are executed
  assert.ok(
    !stdout.includes("INJECTED"),
    "Shell command injection should NOT have executed"
  );

  // Verify passwd file wasn't read (cat /etc/passwd would have succeeded)
  assert.ok(
    !stdout.includes("root:"),
    "Shell pipe injection should NOT have executed"
  );

  // Cleanup
  fs.rmSync(handle.taskDir, { recursive: true, force: true });
});
