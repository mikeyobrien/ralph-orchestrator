import stripAnsi from "strip-ansi";

/**
 * LogStream
 *
 * Captures stdout/stderr from child processes and emits line-by-line events.
 * Handles buffer concatenation for partial lines and supports both
 * streaming callbacks and accumulated output retrieval.
 *
 * Design Notes:
 * - Buffers partial lines until newline is received
 * - Supports both '\n' and '\r\n' line endings
 * - Maintains accumulated output for post-mortem analysis
 * - Optional max buffer size to prevent memory issues with long-running processes
 */

/**
 * Log entry from a stream
 */
export interface LogEntry {
  /** Optional persisted log id */
  id?: number;
  /** The log line content (without newline) */
  line: string;
  /** Timestamp when the line was received */
  timestamp: Date;
  /** Source stream ('stdout' or 'stderr') */
  source: "stdout" | "stderr";
}

/**
 * Callback for receiving log entries
 */
export type LogCallback = (entry: LogEntry) => void;

/**
 * Options for LogStream
 */
export interface LogStreamOptions {
  /** Maximum accumulated output size in bytes (default: 10MB) */
  maxBufferSize?: number;
  /** Callback for each log entry */
  onLine?: LogCallback;
  /** Whether to include timestamps in output (default: true) */
  includeTimestamps?: boolean;
}

/**
 * LogStream
 *
 * Collects and processes output from child process streams.
 */
export class LogStream {
  /** Accumulated stdout lines */
  private stdoutLines: LogEntry[] = [];
  /** Accumulated stderr lines */
  private stderrLines: LogEntry[] = [];
  /** Partial line buffer for stdout */
  private stdoutBuffer: string = "";
  /** Partial line buffer for stderr */
  private stderrBuffer: string = "";
  /** Maximum buffer size */
  private readonly maxBufferSize: number;
  /** Current accumulated size */
  private currentSize: number = 0;
  /** Line callback */
  private readonly onLine?: LogCallback;
  /** Whether to include timestamps */
  private readonly includeTimestamps: boolean;
  /** Whether the stream is closed */
  private closed: boolean = false;

  constructor(options: LogStreamOptions = {}) {
    this.maxBufferSize = options.maxBufferSize ?? 10 * 1024 * 1024; // 10MB
    this.onLine = options.onLine;
    this.includeTimestamps = options.includeTimestamps ?? true;
  }

  /**
   * Process data from stdout
   */
  writeStdout(data: string | Buffer): void {
    if (this.closed) return;
    this.processData(data.toString(), "stdout");
  }

  /**
   * Process data from stderr
   */
  writeStderr(data: string | Buffer): void {
    if (this.closed) return;
    this.processData(data.toString(), "stderr");
  }

  /**
   * Process incoming data, split into lines
   */
  private processData(data: string, source: "stdout" | "stderr"): void {
    const buffer = source === "stdout" ? this.stdoutBuffer : this.stderrBuffer;
    const lines = source === "stdout" ? this.stdoutLines : this.stderrLines;

    // Concatenate with existing buffer
    const combined = buffer + data;

    // Split by newlines (handle both \n and \r\n)
    const parts = combined.split(/\r?\n/);

    // All parts except the last are complete lines
    for (let i = 0; i < parts.length - 1; i++) {
      const entry: LogEntry = {
        line: stripAnsi(parts[i]),
        timestamp: new Date(),
        source,
      };

      // Check buffer size before adding
      if (this.currentSize + entry.line.length <= this.maxBufferSize) {
        lines.push(entry);
        this.currentSize += entry.line.length;
      }

      // Invoke callback
      if (this.onLine) {
        this.onLine(entry);
      }
    }

    // Store the last part as the new buffer (might be partial line)
    if (source === "stdout") {
      this.stdoutBuffer = parts[parts.length - 1];
    } else {
      this.stderrBuffer = parts[parts.length - 1];
    }
  }

  /**
   * Flush any remaining buffered content as final lines.
   * Call this when the stream ends.
   */
  flush(): void {
    // Flush stdout buffer
    if (this.stdoutBuffer) {
      const entry: LogEntry = {
        line: stripAnsi(this.stdoutBuffer),
        timestamp: new Date(),
        source: "stdout",
      };
      this.stdoutLines.push(entry);
      if (this.onLine) {
        this.onLine(entry);
      }
      this.stdoutBuffer = "";
    }

    // Flush stderr buffer
    if (this.stderrBuffer) {
      const entry: LogEntry = {
        line: stripAnsi(this.stderrBuffer),
        timestamp: new Date(),
        source: "stderr",
      };
      this.stderrLines.push(entry);
      if (this.onLine) {
        this.onLine(entry);
      }
      this.stderrBuffer = "";
    }
  }

  /**
   * Close the stream. No more writes accepted.
   */
  close(): void {
    this.flush();
    this.closed = true;
  }

  /**
   * Get all stdout entries
   */
  getStdout(): LogEntry[] {
    return [...this.stdoutLines];
  }

  /**
   * Get all stderr entries
   */
  getStderr(): LogEntry[] {
    return [...this.stderrLines];
  }

  /**
   * Get all entries (stdout + stderr) sorted by timestamp
   */
  getAllEntries(): LogEntry[] {
    return [...this.stdoutLines, ...this.stderrLines].sort(
      (a, b) => a.timestamp.getTime() - b.timestamp.getTime()
    );
  }

  /**
   * Get stdout as a single string
   */
  getStdoutText(): string {
    return this.stdoutLines.map((e) => e.line).join("\n");
  }

  /**
   * Get stderr as a single string
   */
  getStderrText(): string {
    return this.stderrLines.map((e) => e.line).join("\n");
  }

  /**
   * Get combined output as a single string (stdout + stderr interleaved by timestamp)
   */
  getCombinedText(): string {
    const entries = this.getAllEntries();
    if (this.includeTimestamps) {
      return entries
        .map((e) => `[${e.timestamp.toISOString()}] [${e.source}] ${e.line}`)
        .join("\n");
    }
    return entries.map((e) => e.line).join("\n");
  }

  /**
   * Get the number of lines captured
   */
  getLineCount(): { stdout: number; stderr: number; total: number } {
    return {
      stdout: this.stdoutLines.length,
      stderr: this.stderrLines.length,
      total: this.stdoutLines.length + this.stderrLines.length,
    };
  }

  /**
   * Get the current buffer size in bytes
   */
  getBufferSize(): number {
    return this.currentSize;
  }

  /**
   * Check if the stream is closed
   */
  isClosed(): boolean {
    return this.closed;
  }

  /**
   * Clear all accumulated data and reopen for new writes.
   * This allows the stream to be reused after close().
   */
  clear(): void {
    this.stdoutLines = [];
    this.stderrLines = [];
    this.stdoutBuffer = "";
    this.stderrBuffer = "";
    this.currentSize = 0;
    this.closed = false; // Allow reuse after clear
  }
}
