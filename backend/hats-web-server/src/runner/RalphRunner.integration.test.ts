/**
 * Integration test: HatsRunner with ProcessSupervisor
 */

import { test } from "node:test";
import assert from "node:assert";
import { HatsRunner } from "./HatsRunner";
import { ProcessSupervisor } from "./ProcessSupervisor";
import { RunnerState, isTerminalRunnerState } from "./RunnerState";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";

test("HatsRunner uses ProcessSupervisor for spawning", async () => {
  const testDir = path.join(os.tmpdir(), `hats-test-${Date.now()}`);
  const supervisor = new ProcessSupervisor({ runDir: testDir });
  const taskId = `test-${Date.now()}`;

  const runner = new HatsRunner({
    command: "echo",
    baseArgs: ["hello"],
    supervisor,
    taskId,
  });

  try {
    // Run should use supervisor.spawn internally
    const result = await runner.run("test prompt", []);

    // Verify ProcessSupervisor created task directory
    const taskDir = path.join(testDir, taskId);
    assert.ok(fs.existsSync(taskDir), "Task directory should exist");

    // Verify prompt.txt was created (AC-3.2)
    const promptFile = path.join(taskDir, "prompt.txt");
    assert.ok(fs.existsSync(promptFile), "prompt.txt should exist");
    const promptContent = fs.readFileSync(promptFile, "utf-8");
    assert.strictEqual(promptContent, "test prompt", "prompt.txt should contain prompt text");

    // Verify status.json was created and updated (AC-3.6)
    const statusFile = path.join(taskDir, "status.json");
    assert.ok(fs.existsSync(statusFile), "status.json should exist");
    const status = JSON.parse(fs.readFileSync(statusFile, "utf-8"));
    assert.ok(["completed", "failed"].includes(status.state), "status should be terminal");
    assert.ok(status.exitCode !== undefined, "exitCode should be set");

    // Verify PID file was created
    const pidFile = path.join(taskDir, "pid");
    assert.ok(fs.existsSync(pidFile), "pid file should exist");

    // Verify runner result
    assert.ok(result, "Runner should return a result");
    assert.ok(
      isTerminalRunnerState(result.state),
      `Runner should be in terminal state, got: ${result.state}`
    );
  } finally {
    runner.dispose();
    // Cleanup
    if (fs.existsSync(testDir)) {
      fs.rmSync(testDir, { recursive: true, force: true });
    }
  }
});
