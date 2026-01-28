/**
 * ProcessSupervisor
 *
 * Manages detached ralph processes that can survive server restarts.
 * Handles process spawning, reconnection, and state tracking via filesystem.
 *
 * Directory structure:
 *   ~/.ralph/web/runs/{taskId}/
 *     - pid: Process ID
 *     - status.json: ProcessStatus object
 */

import { spawn } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";

export interface ProcessHandle {
  taskId: string;
  pid: number;
  taskDir: string;
  isAlive: boolean;
}

export interface ProcessStatus {
  state: "starting" | "running" | "completed" | "failed";
  startedAt?: string;
  completedAt?: string;
  exitCode?: number;
  signal?: string;
  durationMs?: number;
  error?: string;
}

export interface ProcessSupervisorOptions {
  runDir?: string;
}

export class ProcessSupervisor {
  private readonly runDir: string;

  constructor(options?: ProcessSupervisorOptions) {
    this.runDir = options?.runDir ?? path.join(os.homedir(), ".ralph/web/runs");
  }

  /**
   * Spawn a detached ralph process
   */
  spawn(taskId: string, prompt: string, args: string[], cwd: string): ProcessHandle {
    const taskDir = path.join(this.runDir, taskId);

    // Create task directory
    fs.mkdirSync(taskDir, { recursive: true });

    // Write prompt.txt (AC-3.2)
    fs.writeFileSync(path.join(taskDir, "prompt.txt"), prompt);

    // Write initial status
    const status: ProcessStatus = {
      state: "starting",
      startedAt: new Date().toISOString(),
    };
    fs.writeFileSync(path.join(taskDir, "status.json"), JSON.stringify(status, null, 2));

    // Prepare log file paths
    const stdoutLog = path.join(taskDir, "stdout.log");
    const stderrLog = path.join(taskDir, "stderr.log");

    // Open file descriptors for stdout/stderr redirection
    // Using sync fs operations since we need the fds before spawn
    const stdoutFd = fs.openSync(stdoutLog, "w");
    const stderrFd = fs.openSync(stderrLog, "w");

    // Spawn detached process using array form (no shell injection)
    // SECURITY: Using array form prevents command injection via shell metacharacters
    const child = spawn("ralph", args, {
      cwd,
      detached: true,
      stdio: ["ignore", stdoutFd, stderrFd],
    });

    // Close file descriptors in parent process (child inherits them)
    // IMPORTANT: Must close after spawn, not before
    fs.closeSync(stdoutFd);
    fs.closeSync(stderrFd);

    child.unref();

    if (!child.pid) {
      throw new Error("Failed to spawn process: no PID");
    }

    // Write PID file
    fs.writeFileSync(path.join(taskDir, "pid"), String(child.pid));

    // Update status to running
    status.state = "running";
    fs.writeFileSync(path.join(taskDir, "status.json"), JSON.stringify(status, null, 2));

    // Monitor process exit (AC-3.6)
    const startTime = Date.now();
    child.on("exit", (code, signal) => {
      // Check if directory still exists (may have been cleaned up)
      if (!fs.existsSync(taskDir)) {
        return;
      }

      const exitStatus: ProcessStatus = {
        state: code === 0 ? "completed" : "failed",
        startedAt: status.startedAt,
        completedAt: new Date().toISOString(),
        exitCode: code ?? undefined,
        signal: signal ?? undefined,
        durationMs: Date.now() - startTime,
      };
      fs.writeFileSync(path.join(taskDir, "status.json"), JSON.stringify(exitStatus, null, 2));
    });

    return {
      taskId,
      pid: child.pid,
      taskDir,
      isAlive: true,
    };
  }

  /**
   * Attempt to reconnect to an existing process
   * Returns null if process is dead
   */
  reconnect(taskId: string): ProcessHandle | null {
    const taskDir = path.join(this.runDir, taskId);

    // Check if task directory exists
    if (!fs.existsSync(taskDir)) {
      return null;
    }

    // Read PID file
    const pidFile = path.join(taskDir, "pid");
    if (!fs.existsSync(pidFile)) {
      return null;
    }

    const pidStr = fs.readFileSync(pidFile, "utf-8").trim();
    const pid = parseInt(pidStr, 10);

    if (isNaN(pid)) {
      return null;
    }

    // Check if process is alive
    const alive = this.isAlive(pid);

    if (!alive) {
      return null;
    }

    return {
      taskId,
      pid,
      taskDir,
      isAlive: true,
    };
  }

  /**
   * Check if a process is alive
   */
  isAlive(pid: number): boolean {
    try {
      // kill with signal 0 checks existence without killing
      process.kill(pid, 0);
      return true;
    } catch (err) {
      return false;
    }
  }

  /**
   * Read the current status of a task
   */
  getStatus(taskId: string): ProcessStatus | null {
    const taskDir = path.join(this.runDir, taskId);
    const statusFile = path.join(taskDir, "status.json");

    if (!fs.existsSync(statusFile)) {
      return null;
    }

    try {
      const content = fs.readFileSync(statusFile, "utf-8");
      return JSON.parse(content) as ProcessStatus;
    } catch (err) {
      return null;
    }
  }

  /**
   * Stop a running task by sending SIGTERM, then SIGKILL if needed.
   * Returns true if the process was stopped, false if not found or already dead.
   */
  stop(taskId: string): { success: boolean; signal?: string; error?: string } {
    const taskDir = path.join(this.runDir, taskId);

    // Check if task directory exists
    if (!fs.existsSync(taskDir)) {
      return { success: false, error: "Task not found" };
    }

    // Read PID file
    const pidFile = path.join(taskDir, "pid");
    if (!fs.existsSync(pidFile)) {
      return { success: false, error: "PID file not found" };
    }

    const pidStr = fs.readFileSync(pidFile, "utf-8").trim();
    const pid = parseInt(pidStr, 10);

    if (isNaN(pid)) {
      return { success: false, error: "Invalid PID" };
    }

    // Check if process is alive first
    if (!this.isAlive(pid)) {
      return { success: false, error: "Process already terminated" };
    }

    try {
      // Try SIGTERM first (graceful shutdown)
      process.kill(pid, "SIGTERM");

      // Wait up to 5 seconds for graceful shutdown
      const maxWait = 5000;
      const start = Date.now();
      let alive = true;

      while (alive && Date.now() - start < maxWait) {
        // Poll every 100ms
        const elapsed = Date.now() - start;
        const remaining = Math.max(0, 100 - (elapsed % 100));
        Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, remaining);

        alive = this.isAlive(pid);
      }

      // If still alive, force kill with SIGKILL
      if (alive) {
        process.kill(pid, "SIGKILL");
        // Give it a moment to terminate
        Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100);

        return { success: true, signal: "SIGKILL" };
      }

      return { success: true, signal: "SIGTERM" };
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      // If error is because process doesn't exist, consider it stopped
      if (errorMessage.includes("ESRCH")) {
        return { success: true, signal: "already terminated" };
      }
      return { success: false, error: errorMessage };
    }
  }
}
