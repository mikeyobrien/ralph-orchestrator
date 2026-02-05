/**
 * FileOutputStreamer
 *
 * Streams output from log files in real-time using fs.watch.
 * Supports position-based resume for reconnection scenarios.
 */

import * as fs from "fs";
import * as path from "path";
import stripAnsi from "strip-ansi";

export interface StreamPosition {
  byteOffset: number;
  lineNumber: number;
}

export interface StreamOptions {
  fromPosition?: StreamPosition;
  pollIntervalMs?: number;
}

export type LineCallback = (line: string, source: "stdout" | "stderr") => void;

interface WatchState {
  watcher: fs.FSWatcher;
  position: StreamPosition;
  buffer: string;
}

export class FileOutputStreamer {
  private watchers = new Map<string, { stdout?: WatchState; stderr?: WatchState }>();

  /**
   * Start streaming output from a task's log files
   */
  stream(taskId: string, taskDir: string, callback: LineCallback, options?: StreamOptions): void {
    const stdoutPath = path.join(taskDir, "stdout.log");
    const stderrPath = path.join(taskDir, "stderr.log");

    const state = {
      stdout: this.watchFile(stdoutPath, "stdout", callback, options?.fromPosition),
      stderr: this.watchFile(stderrPath, "stderr", callback, options?.fromPosition),
    };

    this.watchers.set(taskId, state);
  }

  /**
   * Watch a single log file
   */
  private watchFile(
    filePath: string,
    source: "stdout" | "stderr",
    callback: LineCallback,
    fromPosition?: StreamPosition
  ): WatchState | undefined {
    // Create file if it doesn't exist
    if (!fs.existsSync(filePath)) {
      fs.writeFileSync(filePath, "");
    }

    const state: WatchState = {
      watcher: null as any, // Will be set below
      position: fromPosition ?? { byteOffset: 0, lineNumber: 0 },
      buffer: "",
    };

    // Read initial content if resuming from position
    if (state.position.byteOffset > 0) {
      const content = fs.readFileSync(filePath, "utf-8");
      if (content.length > state.position.byteOffset) {
        state.buffer = content.slice(state.position.byteOffset);
      }
    }

    state.watcher = fs.watch(filePath, (eventType) => {
      if (eventType === "change") {
        this.readNewContent(filePath, state, source, callback);
      }
    });

    return state;
  }

  /**
   * Read new content from file and emit lines
   */
  private readNewContent(
    filePath: string,
    state: WatchState,
    source: "stdout" | "stderr",
    callback: LineCallback
  ): void {
    try {
      const stats = fs.statSync(filePath);
      if (stats.size <= state.position.byteOffset) {
        return;
      }

      const stream = fs.createReadStream(filePath, {
        start: state.position.byteOffset,
        encoding: "utf-8",
      });

      let newData = "";
      stream.on("data", (chunk) => {
        newData += chunk;
      });

      stream.on("end", () => {
        const combined = state.buffer + newData;
        const lines = combined.split(/\r?\n/);

        // Emit complete lines
        for (let i = 0; i < lines.length - 1; i++) {
          callback(stripAnsi(lines[i]), source);
          state.position.lineNumber++;
        }

        // Update buffer with partial line
        state.buffer = lines[lines.length - 1];
        state.position.byteOffset += newData.length;
      });
    } catch (err) {
      // File might not exist yet or be temporarily unavailable
    }
  }

  /**
   * Stop streaming for a task
   */
  stop(taskId: string): void {
    const state = this.watchers.get(taskId);
    if (!state) return;

    state.stdout?.watcher.close();
    state.stderr?.watcher.close();
    this.watchers.delete(taskId);
  }

  /**
   * Get current position for resume
   */
  getPosition(taskId: string): StreamPosition | undefined {
    const state = this.watchers.get(taskId);
    return state?.stdout?.position;
  }

  /**
   * Read all historical output
   */
  readAll(taskDir: string): { stdout: string; stderr: string } {
    const stdoutPath = path.join(taskDir, "stdout.log");
    const stderrPath = path.join(taskDir, "stderr.log");

    return {
      stdout: fs.existsSync(stdoutPath) ? fs.readFileSync(stdoutPath, "utf-8") : "",
      stderr: fs.existsSync(stderrPath) ? fs.readFileSync(stderrPath, "utf-8") : "",
    };
  }

  /**
   * Read output from a specific position
   */
  readFrom(
    taskDir: string,
    position: StreamPosition
  ): { stdout: string; stderr: string; newPosition: StreamPosition } {
    const stdoutPath = path.join(taskDir, "stdout.log");
    const stderrPath = path.join(taskDir, "stderr.log");

    let stdout = "";
    let stderr = "";
    const newPosition = { ...position };

    if (fs.existsSync(stdoutPath)) {
      const content = fs.readFileSync(stdoutPath, "utf-8");
      if (content.length > position.byteOffset) {
        stdout = content.slice(position.byteOffset);
        newPosition.byteOffset = content.length;
      }
    }

    if (fs.existsSync(stderrPath)) {
      stderr = fs.readFileSync(stderrPath, "utf-8");
    }

    return { stdout, stderr, newPosition };
  }
}
