/**
 * PromptWriter
 *
 * Manages temporary prompt files for passing to hats run child processes.
 * Handles creation, cleanup, and safe file operations.
 *
 * Design Notes:
 * - Uses OS temp directory for prompt files
 * - Generates unique filenames to avoid conflicts
 * - Supports automatic cleanup on process exit
 * - Can write raw text or structured prompt objects
 */

import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { SettingsService } from "../services/SettingsService";

/**
 * Structured prompt content
 */
export interface PromptContent {
  /** The main prompt/task description */
  task: string;
  /** Optional context to include */
  context?: string;
  /** Optional system instructions */
  system?: string;
  /** Additional metadata */
  metadata?: Record<string, unknown>;
}

/**
 * Options for PromptWriter
 */
export interface PromptWriterOptions {
  /** Base directory for temp files (default: os.tmpdir()) */
  tempDir?: string;
  /** Prefix for temp file names (default: 'hats-prompt-') */
  filePrefix?: string;
  /** Whether to auto-cleanup on process exit (default: true) */
  autoCleanup?: boolean;
  /** Optional SettingsService for persona/hat context injection */
  settingsService?: SettingsService;
}

/**
 * PromptWriter
 *
 * Creates and manages temporary prompt files for hats run invocations.
 */
export class PromptWriter {
  /** Base directory for temp files */
  private readonly tempDir: string;
  /** File name prefix */
  private readonly filePrefix: string;
  /** Whether auto-cleanup is enabled */
  private readonly autoCleanup: boolean;
  /** Optional SettingsService for persona/hat context injection */
  private readonly settingsService?: SettingsService;
  /** Set of created files (for cleanup) */
  private readonly createdFiles: Set<string> = new Set();
  /** Counter for unique file names */
  private fileCounter: number = 0;
  /** Whether cleanup handler is registered */
  private cleanupRegistered: boolean = false;

  constructor(options: PromptWriterOptions = {}) {
    this.tempDir = options.tempDir ?? os.tmpdir();
    this.filePrefix = options.filePrefix ?? "hats-prompt-";
    this.autoCleanup = options.autoCleanup ?? true;
    this.settingsService = options.settingsService;

    if (this.autoCleanup) {
      this.registerCleanupHandler();
    }
  }

  /**
   * Build context prefix from persona and hat settings.
   * Returns empty string if no SettingsService is configured or no context is available.
   */
  private buildContextPrefix(): string {
    if (!this.settingsService) {
      return "";
    }

    const parts: string[] = [];

    // Get persona context
    const persona = this.settingsService.getCurrentPersonaDefinition();
    if (persona?.systemPrompt) {
      parts.push(`<persona>\n${persona.systemPrompt}\n</persona>`);
    }

    // Get hat context
    const hat = this.settingsService.getActiveHatDefinition();
    if (hat?.instructions) {
      parts.push(`<hat name="${hat.name}">\n${hat.instructions}\n</hat>`);
    } else if (hat?.description) {
      // Fallback to description if no instructions
      parts.push(`<hat name="${hat.name}">\n${hat.description}\n</hat>`);
    }

    if (parts.length === 0) {
      return "";
    }

    return parts.join("\n\n") + "\n\n";
  }

  /**
   * Generate a unique file path
   */
  private generateFilePath(): string {
    const timestamp = Date.now();
    const counter = ++this.fileCounter;
    const filename = `${this.filePrefix}${timestamp}-${counter}.txt`;
    return path.join(this.tempDir, filename);
  }

  /**
   * Register process exit handler for cleanup
   */
  private registerCleanupHandler(): void {
    if (this.cleanupRegistered) return;

    const cleanup = () => {
      this.cleanupAll();
    };

    process.on("exit", cleanup);
    process.on("SIGINT", () => {
      cleanup();
      process.exit(130);
    });
    process.on("SIGTERM", () => {
      cleanup();
      process.exit(143);
    });

    this.cleanupRegistered = true;
  }

  /**
   * Write a text prompt to a temporary file.
   * If a SettingsService is configured, persona and hat context are prepended.
   *
   * @param content - The prompt text content
   * @returns The file path
   */
  writeText(content: string): string {
    const filePath = this.generateFilePath();

    // Ensure temp directory exists
    if (!fs.existsSync(this.tempDir)) {
      fs.mkdirSync(this.tempDir, { recursive: true });
    }

    // Build content with context prefix
    const contextPrefix = this.buildContextPrefix();
    const fullContent = contextPrefix + content;

    // Write the file
    fs.writeFileSync(filePath, fullContent, "utf-8");
    this.createdFiles.add(filePath);

    return filePath;
  }

  /**
   * Write a structured prompt to a temporary file.
   * If a SettingsService is configured, persona and hat context are prepended
   * before any other sections.
   *
   * @param prompt - The structured prompt content
   * @returns The file path
   */
  writePrompt(prompt: PromptContent): string {
    const parts: string[] = [];

    // Add persona/hat context first (if SettingsService is configured)
    const contextPrefix = this.buildContextPrefix();
    if (contextPrefix) {
      parts.push(contextPrefix.trim());
    }

    // Add system instructions if present
    if (prompt.system) {
      parts.push(`<system>\n${prompt.system}\n</system>\n`);
    }

    // Add context if present
    if (prompt.context) {
      parts.push(`<context>\n${prompt.context}\n</context>\n`);
    }

    // Add the main task
    parts.push(prompt.task);

    // Add metadata as a comment if present
    if (prompt.metadata && Object.keys(prompt.metadata).length > 0) {
      parts.push(`\n<!-- metadata: ${JSON.stringify(prompt.metadata)} -->`);
    }

    // Use writeTextRaw to avoid double context injection
    return this.writeTextRaw(parts.join("\n"));
  }

  /**
   * Write raw text content without context injection.
   * Internal method used by writePrompt to avoid double injection.
   *
   * @param content - The raw content to write
   * @returns The file path
   */
  private writeTextRaw(content: string): string {
    const filePath = this.generateFilePath();

    // Ensure temp directory exists
    if (!fs.existsSync(this.tempDir)) {
      fs.mkdirSync(this.tempDir, { recursive: true });
    }

    // Write the file without context prefix
    fs.writeFileSync(filePath, content, "utf-8");
    this.createdFiles.add(filePath);

    return filePath;
  }

  /**
   * Read a prompt file back
   *
   * @param filePath - Path to the prompt file
   * @returns The file content
   */
  read(filePath: string): string {
    return fs.readFileSync(filePath, "utf-8");
  }

  /**
   * Delete a specific prompt file
   *
   * @param filePath - Path to delete
   * @returns true if deleted, false if not found
   */
  delete(filePath: string): boolean {
    if (!this.createdFiles.has(filePath)) {
      return false;
    }

    try {
      if (fs.existsSync(filePath)) {
        fs.unlinkSync(filePath);
      }
      this.createdFiles.delete(filePath);
      return true;
    } catch (err) {
      console.debug(`[PromptWriter] Failed to delete prompt file ${filePath}:`, err);
      return false;
    }
  }

  /**
   * Clean up all created prompt files
   *
   * @returns Number of files cleaned up
   */
  cleanupAll(): number {
    let cleaned = 0;

    for (const filePath of this.createdFiles) {
      try {
        if (fs.existsSync(filePath)) {
          fs.unlinkSync(filePath);
          cleaned++;
        }
      } catch (err) {
        console.debug(`[PromptWriter] Failed to clean up ${filePath}:`, err);
      }
    }

    this.createdFiles.clear();
    return cleaned;
  }

  /**
   * Get the list of created files
   */
  getCreatedFiles(): string[] {
    return Array.from(this.createdFiles);
  }

  /**
   * Get the number of active prompt files
   */
  getActiveCount(): number {
    return this.createdFiles.size;
  }

  /**
   * Check if a file was created by this writer
   */
  isOwnedFile(filePath: string): boolean {
    return this.createdFiles.has(filePath);
  }
}
