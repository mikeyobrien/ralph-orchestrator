/**
 * LoopsManager
 *
 * Manages ralph loops for parallel task execution.
 * Handles periodic merge queue processing to resolve git conflicts
 * that occur when multiple worktree loops complete concurrently.
 *
 * Key responsibilities:
 * - Periodically run `ralph loops process` to clear merge queue
 * - List and query loop status
 * - Prune stale loops from crashed processes
 * - Retry failed merges or discard stuck loops
 */

import { spawn } from "child_process";
import { EventEmitter } from "events";

export interface LoopStatus {
  id: string;
  status: "running" | "completed" | "failed" | "merging" | "stuck" | "queued";
  location: string;
  pid?: number;
  prompt?: string;
}

/**
 * State of the merge button for a loop.
 * Active means merge can proceed, blocked means it cannot.
 */
export interface MergeButtonState {
  state: "active" | "blocked";
  reason?: string;
}

export interface LoopsManagerOptions {
  /** How often to process merge queue (default: 30s) */
  processIntervalMs?: number;
  /** Path to ralph executable (default: "ralph") */
  ralphPath?: string;
  /** Workspace root directory (cwd for ralph subprocesses) */
  workspaceRoot?: string;
}

/**
 * Manages ralph loops for parallel execution.
 * Handles merge queue processing and conflict detection.
 */
export class LoopsManager extends EventEmitter {
  private processingTimer?: NodeJS.Timeout;
  private readonly processIntervalMs: number;
  private readonly ralphPath: string;
  private readonly workspaceRoot?: string;

  /** Event types emitted by LoopsManager */
  static readonly Events = {
    PROCESSED: "processed",
    ERROR: "error",
    PRUNED: "pruned",
  } as const;

  constructor(options: LoopsManagerOptions = {}) {
    super();
    this.processIntervalMs = options.processIntervalMs ?? 30000; // Default: 30s
    this.ralphPath = options.ralphPath ?? "ralph";
    this.workspaceRoot = options.workspaceRoot;
  }

  /**
   * Start periodic merge queue processing
   */
  start(): void {
    if (this.processingTimer) {
      console.log("LoopsManager already started");
      return;
    }

    this.processingTimer = setInterval(() => {
      this.processMergeQueue().catch((err) => {
        console.error("LoopsManager process error:", err);
        this.emit(LoopsManager.Events.ERROR, err);
      });
    }, this.processIntervalMs);

    console.log(`LoopsManager started (processing every ${this.processIntervalMs}ms)`);
    this.emit("started");
  }

  /**
   * Stop periodic processing
   */
  stop(): void {
    if (this.processingTimer) {
      clearInterval(this.processingTimer);
      this.processingTimer = undefined;
      console.log("LoopsManager stopped");
      this.emit("stopped");
    }
  }

  /**
   * Check if LoopsManager is currently running
   */
  isRunning(): boolean {
    return this.processingTimer !== undefined;
  }

  /**
   * Process the merge queue (runs ralph loops process)
   * This command handles pending merges after worktree loops complete.
   */
  async processMergeQueue(): Promise<void> {
    try {
      await this.runRalphCommand(["loops", "process"]);
      this.lastProcessedAt = new Date().toISOString();
      this.emit(LoopsManager.Events.PROCESSED);
    } catch (error) {
      this.emit(LoopsManager.Events.ERROR, error);
      throw error;
    }
  }

  /**
   * List all loops and their status
   */
  async listLoops(): Promise<LoopStatus[]> {
    const output = await this.runRalphCommand(["loops", "list", "--json"]);
    try {
      const loops = JSON.parse(output);
      return loops;
    } catch {
      // If JSON parsing fails, return empty array
      // (ralph loops list --json may not be available in all versions)
      return [];
    }
  }

  /**
   * Prune stale loops from crashed processes
   */
  async pruneStale(): Promise<void> {
    try {
      await this.runRalphCommand(["loops", "prune"]);
      this.emit(LoopsManager.Events.PRUNED);
    } catch (error) {
      this.emit(LoopsManager.Events.ERROR, error);
      throw error;
    }
  }

  /**
   * Retry a failed merge with optional user steering input.
   * If steeringInput is provided, it's written to .ralph/merge-steering.txt
   * for the merge-ralph process to read and incorporate into its strategy.
   */
  async retryMerge(loopId: string, steeringInput?: string): Promise<void> {
    // Write steering input to file for merge-ralph to read
    if (steeringInput?.trim()) {
      const steeringPath = `${process.cwd()}/.ralph/merge-steering.txt`;
      const fs = await import("fs/promises");
      await fs.writeFile(steeringPath, steeringInput.trim(), "utf-8");
    }

    await this.runRalphCommand(["loops", "retry", loopId]);
  }

  /**
   * Discard a stuck loop
   */
  async discardLoop(loopId: string): Promise<void> {
    // -y skips CLI confirmation prompt (web UI already confirmed)
    await this.runRalphCommand(["loops", "discard", "-y", loopId]);
  }

  /**
   * Stop a running loop
   */
  async stopLoop(loopId: string, force?: boolean): Promise<void> {
    const args = ["loops", "stop", loopId];
    if (force) {
      args.push("--force");
    }
    await this.runRalphCommand(args);
  }

  /**
   * Force merge a loop (triggers immediate merge)
   */
  async mergeLoop(loopId: string, force?: boolean): Promise<void> {
    const args = ["loops", "merge", loopId];
    if (force) {
      args.push("--force");
    }
    await this.runRalphCommand(args);
  }

  /**
   * Get merge button state for a loop.
   * Returns whether merge is active (can proceed) or blocked (with reason).
   */
  async getMergeButtonState(loopId: string): Promise<MergeButtonState> {
    const output = await this.runRalphCommand(["loops", "merge-button-state", loopId]);
    const result = JSON.parse(output);
    return result as MergeButtonState;
  }

  /**
   * Get manager status info
   */
  getStatus(): { running: boolean; intervalMs: number; lastProcessedAt?: string } {
    return {
      running: this.isRunning(),
      intervalMs: this.processIntervalMs,
      lastProcessedAt: this.lastProcessedAt,
    };
  }

  private lastProcessedAt?: string;

  /**
   * Run a ralph command and return stdout
   */
  private runRalphCommand(args: string[]): Promise<string> {
    return new Promise((resolve, reject) => {
      const proc = spawn(this.ralphPath, args, {
        stdio: ["ignore", "pipe", "pipe"],
        ...(this.workspaceRoot ? { cwd: this.workspaceRoot } : {}),
      });

      let stdout = "";
      let stderr = "";

      proc.stdout?.on("data", (data) => {
        stdout += data.toString();
      });

      proc.stderr?.on("data", (data) => {
        stderr += data.toString();
      });

      proc.on("close", (code) => {
        if (code === 0) {
          resolve(stdout);
        } else {
          reject(new Error(`ralph ${args.join(" ")} failed (exit ${code}): ${stderr}`));
        }
      });

      proc.on("error", (error) => {
        reject(error);
      });
    });
  }
}
