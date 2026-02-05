/**
 * HatsRunner
 *
 * Spawns and manages hats run child processes. This is the execution engine
 * that bridges the Dispatcher's task model with actual CLI subprocess invocation.
 *
 * Lifecycle:
 *   IDLE → start() → SPAWNING → spawn complete → RUNNING → exit → COMPLETED/FAILED
 *                                                      ↓
 *                                              stop() → CANCELLED
 *
 * Integration:
 * - Can be registered as a TaskHandler with the Dispatcher
 * - Emits events for progress tracking via EventBus
 * - Uses LogStream to capture stdout/stderr
 * - Uses PromptWriter to pass prompt content to subprocess
 *
 * Design Notes:
 * - Single process per HatsRunner instance
 * - Supports cancellation via AbortSignal
 * - Graceful shutdown with SIGTERM, then SIGKILL after timeout
 * - Configurable command and arguments
 */

import { spawn, ChildProcess, SpawnOptions } from "child_process";
import { EventEmitter } from "events";
import { RunnerState, isTerminalRunnerState, isValidRunnerTransition } from "./RunnerState";
import { LogStream, LogEntry, LogCallback } from "./LogStream";
import { PromptWriter, PromptContent } from "./PromptWriter";
import { ProcessSupervisor, ProcessHandle } from "./ProcessSupervisor";
import { FileOutputStreamer } from "./FileOutputStreamer";

/**
 * Configuration options for HatsRunner
 */
export interface HatsRunnerOptions {
  /** Command to execute (default: 'hats') */
  command?: string;
  /** Base arguments (default: ['run']) */
  baseArgs?: string[];
  /** Working directory for the subprocess */
  cwd?: string;
  /** Environment variables (merged with process.env) */
  env?: Record<string, string>;
  /** Graceful stop timeout in ms before SIGKILL (default: 5000) */
  gracefulTimeoutMs?: number;
  /** Maximum output buffer size (default: 10MB) */
  maxOutputSize?: number;
  /** Shell to use (default: false - no shell) */
  shell?: boolean;
  /** Callback for log output */
  onOutput?: LogCallback;
  /** ProcessSupervisor instance (optional, creates default if not provided) */
  supervisor?: ProcessSupervisor;
  /** FileOutputStreamer instance (optional, creates default if not provided) */
  outputStreamer?: FileOutputStreamer;
  /** Task ID for process tracking (optional, generates UUID if not provided) */
  taskId?: string;
}

/**
 * Result of a runner execution
 */
export interface RunnerResult {
  /** Final state */
  state: RunnerState;
  /** Exit code (if process exited normally) */
  exitCode?: number;
  /** Signal that killed the process (if applicable) */
  signal?: string;
  /** Captured stdout */
  stdout: string;
  /** Captured stderr */
  stderr: string;
  /** Combined output (interleaved by timestamp) */
  combined: string;
  /** Duration in milliseconds */
  durationMs: number;
  /** Error message (if failed) */
  error?: string;
}

/**
 * Events emitted by HatsRunner
 */
export interface HatsRunnerEvents {
  /** State changed */
  stateChange: (state: RunnerState, previousState: RunnerState) => void;
  /** Output line received */
  output: (entry: LogEntry) => void;
  /** Process spawned */
  spawned: (pid: number) => void;
  /** Process completed */
  completed: (result: RunnerResult) => void;
  /** Error occurred */
  error: (error: Error) => void;
}

/**
 * HatsRunner
 *
 * Manages the lifecycle of a hats run child process.
 */
export class HatsRunner extends EventEmitter {
  /** Current state */
  private _state: RunnerState = RunnerState.IDLE;
  /** Child process reference */
  private process?: ChildProcess;
  /** Process handle from supervisor */
  private processHandle?: ProcessHandle;
  /** Log stream for output capture */
  private logStream: LogStream;
  /** Prompt writer for temp files */
  private promptWriter: PromptWriter;
  /** Process supervisor for detached process management */
  private supervisor: ProcessSupervisor;
  /** File output streamer for log file monitoring */
  private outputStreamer: FileOutputStreamer;
  /** Current prompt file path */
  private promptFilePath?: string;
  /** Start timestamp */
  private startedAt?: Date;
  /** Resolve function for run() promise */
  private runResolve?: (result: RunnerResult) => void;
  /** Configuration */
  private readonly taskId: string;
  private readonly command: string;
  private readonly baseArgs: string[];
  private readonly cwd?: string;
  private readonly env?: Record<string, string>;
  private readonly gracefulTimeoutMs: number;
  private readonly maxOutputSize: number;
  private readonly shell: boolean;
  private readonly onOutput?: LogCallback;

  constructor(options: HatsRunnerOptions = {}) {
    super();

    // Prevent unhandled 'error' events from crashing the process
    // Errors are still emitted for listeners that want them, but
    // if no listener is attached, the error is captured in the result
    this.on("error", () => {
      // Intentionally empty - prevents Node.js from throwing
      // The error is captured in handleError() and returned in RunnerResult
    });

    this.taskId =
      options.taskId ?? `runner-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
    this.command = options.command ?? "hats";
    this.baseArgs = options.baseArgs ?? ["run"];
    this.cwd = options.cwd;
    this.env = options.env;
    this.gracefulTimeoutMs = options.gracefulTimeoutMs ?? 5000;
    this.maxOutputSize = options.maxOutputSize ?? 10 * 1024 * 1024;
    this.shell = options.shell ?? false;
    this.onOutput = options.onOutput;
    this.supervisor = options.supervisor ?? new ProcessSupervisor();
    this.outputStreamer = options.outputStreamer ?? new FileOutputStreamer();

    this.logStream = new LogStream({
      maxBufferSize: this.maxOutputSize,
      onLine: (entry) => {
        this.emit("output", entry);
        if (this.onOutput) {
          this.onOutput(entry);
        }
      },
    });

    this.promptWriter = new PromptWriter();
  }

  /**
   * Get the current state
   */
  get state(): RunnerState {
    return this._state;
  }

  /**
   * Get the child process PID (if running)
   */
  get pid(): number | undefined {
    return this.process?.pid;
  }

  /**
   * Transition to a new state
   */
  private setState(newState: RunnerState): void {
    if (!isValidRunnerTransition(this._state, newState)) {
      throw new Error(`Invalid state transition: ${this._state} -> ${newState}`);
    }

    const previousState = this._state;
    this._state = newState;
    this.emit("stateChange", newState, previousState);
  }

  /**
   * Run hats with a text prompt
   *
   * @param prompt - The prompt text or structured content
   * @param additionalArgs - Additional CLI arguments
   * @param signal - Optional AbortSignal for cancellation
   * @returns Promise resolving to the execution result
   */
  async run(
    prompt: string | PromptContent,
    additionalArgs: string[] = [],
    signal?: AbortSignal
  ): Promise<RunnerResult> {
    // Check current state
    if (this._state !== RunnerState.IDLE) {
      throw new Error(`Cannot start runner in state: ${this._state}`);
    }

    // Reset for new run
    this.logStream.clear();
    this.startedAt = new Date();

    // Extract prompt text for ProcessSupervisor
    const promptText = typeof prompt === "string" ? prompt : JSON.stringify(prompt);

    // Write prompt to temp file for -P flag
    if (typeof prompt === "string") {
      this.promptFilePath = this.promptWriter.writeText(prompt);
    } else {
      this.promptFilePath = this.promptWriter.writePrompt(prompt);
    }

    // Build arguments (use -P flag for prompt file)
    const args = [...this.baseArgs, "-P", this.promptFilePath, ...additionalArgs];

    // Transition to SPAWNING
    this.setState(RunnerState.SPAWNING);

    return new Promise<RunnerResult>((resolve, reject) => {
      this.runResolve = resolve;

      try {
        // Spawn via ProcessSupervisor (writes to log files)
        this.processHandle = this.supervisor.spawn(
          this.taskId,
          promptText,
          args,
          this.cwd ?? process.cwd(),
          this.command
        );

        // Start streaming output from log files
        this.outputStreamer.stream(this.taskId, this.processHandle.taskDir, (line, source) => {
          // Write to LogStream for buffering
          if (source === "stdout") {
            this.logStream.writeStdout(Buffer.from(line + "\n"));
          } else {
            this.logStream.writeStderr(Buffer.from(line + "\n"));
          }
        });

        // Monitor process status by polling
        const checkInterval = setInterval(() => {
          if (!this.processHandle) {
            clearInterval(checkInterval);
            return;
          }

          // Check if process is still alive
          if (!this.supervisor.isAlive(this.processHandle.pid)) {
            clearInterval(checkInterval);
            // Read final status
            const status = this.supervisor.getStatus(this.taskId);
            this.handleExit(status?.exitCode ?? null, (status?.signal as NodeJS.Signals) ?? null);
          }
        }, 500);

        // Transition to RUNNING
        this.setState(RunnerState.RUNNING);
        this.emit("spawned", this.processHandle.pid);

        // Set up abort signal handler
        if (signal) {
          if (signal.aborted) {
            // Already aborted
            this.stop();
          } else {
            signal.addEventListener("abort", () => {
              this.stop();
            });
          }
        }
      } catch (err) {
        this.handleError(err instanceof Error ? err : new Error(String(err)));
        reject(err);
      }
    });
  }

  /**
   * Stop the running process
   *
   * @param force - If true, skip graceful shutdown and SIGKILL immediately
   */
  async stop(force: boolean = false): Promise<void> {
    if (!this.processHandle || isTerminalRunnerState(this._state)) {
      return;
    }

    const signal = force ? "SIGKILL" : "SIGTERM";

    try {
      process.kill(this.processHandle.pid, signal);

      if (!force) {
        // Schedule SIGKILL if process doesn't exit
        setTimeout(() => {
          if (this.processHandle && !isTerminalRunnerState(this._state)) {
            try {
              process.kill(this.processHandle.pid, "SIGKILL");
            } catch (err) {
              // Process may have already exited
            }
          }
        }, this.gracefulTimeoutMs);
      }
    } catch (err) {
      // Process may have already exited
    }
  }

  /**
   * Handle process exit
   */
  private handleExit(code: number | null, signal: NodeJS.Signals | null): void {
    // Stop output streaming
    this.outputStreamer.stop(this.taskId);

    // Flush any remaining output
    this.logStream.close();

    // Clean up prompt file
    if (this.promptFilePath) {
      this.promptWriter.delete(this.promptFilePath);
      this.promptFilePath = undefined;
    }

    // Calculate duration
    const durationMs = this.startedAt ? Date.now() - this.startedAt.getTime() : 0;

    // Determine final state
    let finalState: RunnerState;
    let error: string | undefined;

    if (signal === "SIGTERM" || signal === "SIGKILL") {
      finalState = RunnerState.CANCELLED;
    } else if (code === 0) {
      finalState = RunnerState.COMPLETED;
    } else {
      finalState = RunnerState.FAILED;
      error = `Process exited with code ${code}`;
    }

    // Only transition if not already terminal (could have errored during spawn)
    if (!isTerminalRunnerState(this._state)) {
      this.setState(finalState);
    }

    // Build result
    const result: RunnerResult = {
      state: this._state,
      exitCode: code ?? undefined,
      signal: signal ?? undefined,
      stdout: this.logStream.getStdoutText(),
      stderr: this.logStream.getStderrText(),
      combined: this.logStream.getCombinedText(),
      durationMs,
      error,
    };

    // Clear process reference
    this.process = undefined;
    this.processHandle = undefined;

    // Emit completion
    this.emit("completed", result);

    // Resolve the run() promise
    if (this.runResolve) {
      this.runResolve(result);
      this.runResolve = undefined;
    }
  }

  /**
   * Handle spawn/runtime errors
   */
  private handleError(err: Error): void {
    // Stop output streaming
    this.outputStreamer.stop(this.taskId);

    // Flush any output we might have
    this.logStream.close();

    // Clean up prompt file
    if (this.promptFilePath) {
      this.promptWriter.delete(this.promptFilePath);
      this.promptFilePath = undefined;
    }

    // Calculate duration
    const durationMs = this.startedAt ? Date.now() - this.startedAt.getTime() : 0;

    // Transition to FAILED if not already terminal
    if (!isTerminalRunnerState(this._state)) {
      this.setState(RunnerState.FAILED);
    }

    // Emit error
    this.emit("error", err);

    // Build result
    const result: RunnerResult = {
      state: RunnerState.FAILED,
      stdout: this.logStream.getStdoutText(),
      stderr: this.logStream.getStderrText(),
      combined: this.logStream.getCombinedText(),
      durationMs,
      error: err.message,
    };

    // Clear process reference
    this.process = undefined;
    this.processHandle = undefined;

    // Emit completion
    this.emit("completed", result);

    // Resolve the run() promise
    if (this.runResolve) {
      this.runResolve(result);
      this.runResolve = undefined;
    }
  }

  /**
   * Get the current output without waiting for completion
   */
  getOutput(): { stdout: string; stderr: string; combined: string } {
    return {
      stdout: this.logStream.getStdoutText(),
      stderr: this.logStream.getStderrText(),
      combined: this.logStream.getCombinedText(),
    };
  }

  /**
   * Get output line count
   */
  getLineCount(): { stdout: number; stderr: number; total: number } {
    return this.logStream.getLineCount();
  }

  /**
   * Check if the runner is in a terminal state
   */
  isTerminal(): boolean {
    return isTerminalRunnerState(this._state);
  }

  /**
   * Check if the runner is currently running
   */
  isRunning(): boolean {
    return this._state === RunnerState.RUNNING;
  }

  /**
   * Reset the runner to IDLE state for reuse.
   * Only works if in a terminal state.
   */
  reset(): void {
    if (!isTerminalRunnerState(this._state) && this._state !== RunnerState.IDLE) {
      throw new Error(`Cannot reset runner in state: ${this._state}`);
    }

    this.process = undefined;
    this.promptFilePath = undefined;
    this.startedAt = undefined;
    this.runResolve = undefined;
    this.logStream.clear();
    this._state = RunnerState.IDLE;
  }

  /**
   * Clean up resources
   */
  dispose(): void {
    // Force stop if running
    if (this.processHandle && !isTerminalRunnerState(this._state)) {
      try {
        process.kill(this.processHandle.pid, "SIGKILL");
      } catch (err) {
        // Process may have already exited
      }
    }

    // Stop output streaming
    this.outputStreamer.stop(this.taskId);

    // Clean up prompt files
    this.promptWriter.cleanupAll();

    // Close log stream
    this.logStream.close();

    // Remove all listeners
    this.removeAllListeners();
  }
}
