/**
 * PlanningService
 *
 * Manages planning session lifecycle for the web-based planning page.
 * Handles session creation, listing, response submission, and file operations.
 *
 * Integration points:
 * - Creates session directories in .hats/planning-sessions/
 * - Spawns Hats processes with planning preset
 * - Manages conversation file (JSONL format)
 * - Provides session metadata to frontend
 */

import * as fs from "node:fs/promises";
import * as path from "node:path";
import { v4 as uuidv4 } from "uuid";
import { spawn, ChildProcess } from "child_process";

/**
 * Status of a planning session.
 */
export enum SessionStatus {
  Active = "active",
  WaitingForInput = "waiting_for_input",
  Completed = "completed",
  TimedOut = "timed_out",
  Failed = "failed",
  Paused = "paused",
}

/**
 * Session metadata from the session.json file.
 */
export interface SessionMetadata {
  id: string;
  prompt: string;
  status: SessionStatus;
  created_at: string;
  updated_at: string;
  iterations: number;
  config?: string;
}

/**
 * A single entry in the planning conversation (backend format).
 */
export interface ConversationEntry {
  type: "user_prompt" | "user_response";
  id: string;
  text: string;
  ts: string;
}

/**
 * Frontend-compatible conversation entry format.
 */
export interface FrontendConversationEntry {
  type: "prompt" | "response";
  id: string;
  content: string;
  timestamp: string;
}

/**
 * Full session details with conversation history (frontend format).
 */
export interface PlanningSessionDetail {
  id: string;
  prompt: string;
  status: string;
  title?: string;
  createdAt: string;
  updatedAt: string;
  completedAt?: string;
  conversation: FrontendConversationEntry[];
  artifacts?: string[];
  messageCount?: number;
}

/**
 * Summary info for session lists (frontend format).
 */
export interface PlanningSessionSummary {
  id: string;
  title?: string;
  prompt: string;
  status: string;
  createdAt: string;
  updatedAt: string;
  messageCount?: number;
  iterations?: number;
}

/**
 * Configuration options for the PlanningService.
 */
export interface PlanningServiceOptions {
  /** Root directory of the Hats project */
  workspaceRoot: string;
  /** Path to hats binary (default: "hats") */
  hatsPath?: string;
  /** Default timeout for user responses (seconds, default: 300) */
  defaultTimeoutSeconds?: number;
}

/**
 * Convert backend conversation entry to frontend format.
 */
function toFrontendEntry(entry: ConversationEntry): FrontendConversationEntry {
  return {
    type: entry.type === "user_prompt" ? "prompt" : "response",
    id: entry.id,
    content: entry.text,
    timestamp: entry.ts,
  };
}

/**
 * Convert backend status to frontend status string.
 */
function toFrontendStatus(status: SessionStatus): string {
  // Map waiting_for_input to paused for the frontend
  if (status === SessionStatus.WaitingForInput) {
    return "paused";
  }
  return status;
}

/**
 * Generate a title from the prompt.
 */
function generateTitle(prompt: string): string {
  const trimmed = prompt.trim();
  if (trimmed.length <= 60) {
    return trimmed;
  }
  return trimmed.substring(0, 57) + "...";
}

/**
 * Represents a Hats event from the events JSONL file.
 */
interface HatsEvent {
  topic: string;
  payload: unknown;
  ts: string;
}

/**
 * Represents the user.prompt payload format.
 */
interface UserPromptPayload {
  id: string;
  question: string;
}

/**
 * Service for managing planning sessions.
 */
export class PlanningService {
  private readonly workspaceRoot: string;
  private readonly hatsPath: string;
  private readonly sessionsDir: string;
  private readonly defaultTimeoutSeconds: number;
  private readonly runningProcesses = new Map<string, ChildProcess>();
  private readonly eventPollers = new Map<string, NodeJS.Timeout>();
  private readonly processedEventTimestamps = new Map<string, Set<string>>();

  constructor(options: PlanningServiceOptions) {
    this.workspaceRoot = options.workspaceRoot;
    this.hatsPath = options.hatsPath ?? "hats";
    this.sessionsDir = path.join(this.workspaceRoot, ".hats", "planning-sessions");
    this.defaultTimeoutSeconds = options.defaultTimeoutSeconds ?? 300;
    // TODO: use defaultTimeoutSeconds for request timeout handling
    void this.defaultTimeoutSeconds;
  }

  /**
   * Get all planning sessions as summaries.
   */
  async listSessions(): Promise<PlanningSessionSummary[]> {
    await this.ensureSessionsDir();

    const entries = await fs.readdir(this.sessionsDir, { withFileTypes: true });
    const sessions: PlanningSessionSummary[] = [];

    for (const entry of entries) {
      if (entry.isDirectory()) {
        const metadataPath = path.join(this.sessionsDir, entry.name, "session.json");
        try {
          const content = await fs.readFile(metadataPath, "utf-8");
          const metadata: SessionMetadata = JSON.parse(content);

          // Count messages in conversation
          const conversationPath = path.join(this.sessionsDir, entry.name, "conversation.jsonl");
          let messageCount = 0;
          try {
            const convContent = await fs.readFile(conversationPath, "utf-8");
            messageCount = convContent.trim().split("\n").filter((l: string) => l.trim()).length;
          } catch (err) {
            // Expected if conversation file doesn't exist yet
            console.warn(`[PlanningService] Could not read conversation file for session ${entry.name}:`, err);
          }

          sessions.push({
            id: metadata.id,
            title: generateTitle(metadata.prompt),
            prompt: metadata.prompt,
            status: toFrontendStatus(metadata.status),
            createdAt: metadata.created_at,
            updatedAt: metadata.updated_at,
            messageCount,
            iterations: metadata.iterations,
          });
        } catch (err) {
          console.warn(`[PlanningService] Skipping session ${entry.name} due to invalid metadata:`, err);
        }
      }
    }

    // Sort by updated_at descending (most recent first)
    sessions.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
    return sessions;
  }

  /**
   * Get a specific session with full details.
   */
  async getSession(sessionId: string): Promise<PlanningSessionDetail> {
    const sessionDir = path.join(this.sessionsDir, sessionId);

    // Load metadata
    const metadataPath = path.join(sessionDir, "session.json");
    const metadataContent = await fs.readFile(metadataPath, "utf-8");
    const metadata: SessionMetadata = JSON.parse(metadataContent);

    // Load conversation
    const conversationPath = path.join(sessionDir, "conversation.jsonl");
    const conversation: FrontendConversationEntry[] = [];

    try {
      const conversationContent = await fs.readFile(conversationPath, "utf-8");
      const lines = conversationContent.trim().split("\n");
      const entries = lines
        .filter((line: string) => line.trim().length > 0)
        .map((line: string) => JSON.parse(line) as ConversationEntry);

      // Convert to frontend format
      for (const entry of entries) {
        conversation.push(toFrontendEntry(entry));
      }
    } catch (err) {
      console.warn(`[PlanningService:${sessionId}] Could not load conversation file:`, err);
    }

    // List artifacts if any
    const artifactsDir = path.join(sessionDir, "artifacts");
    let artifacts: string[] = [];
    try {
      const artifactEntries = await fs.readdir(artifactsDir);
      artifacts = artifactEntries.filter((e: string) => !e.startsWith("."));
    } catch (err) {
      console.warn(`[PlanningService:${sessionId}] Could not read artifacts directory:`, err);
    }

    const isCompleted = metadata.status === SessionStatus.Completed;

    return {
      id: metadata.id,
      prompt: metadata.prompt,
      title: generateTitle(metadata.prompt),
      status: toFrontendStatus(metadata.status),
      createdAt: metadata.created_at,
      updatedAt: metadata.updated_at,
      completedAt: isCompleted ? metadata.updated_at : undefined,
      conversation,
      artifacts,
      messageCount: conversation.length,
    };
  }

  /**
   * Start a new planning session.
   */
  async startSession(prompt: string): Promise<{ sessionId: string }> {
    await this.ensureSessionsDir();

    // Generate session ID (timestamp-based with random suffix)
    const sessionId = this.generateSessionId();
    const sessionDir = path.join(this.sessionsDir, sessionId);

    // Create session directory
    await fs.mkdir(sessionDir, { recursive: true });

    // Create artifacts directory
    await fs.mkdir(path.join(sessionDir, "artifacts"), { recursive: true });

    // Create metadata
    const now = new Date().toISOString();
    const metadata: SessionMetadata = {
      id: sessionId,
      prompt,
      status: SessionStatus.Active,
      created_at: now,
      updated_at: now,
      iterations: 0,
    };

    const metadataPath = path.join(sessionDir, "session.json");
    await fs.writeFile(metadataPath, JSON.stringify(metadata, null, 2));

    // Create empty conversation file
    const conversationPath = path.join(sessionDir, "conversation.jsonl");
    await fs.writeFile(conversationPath, "");

    // Spawn hats process with planning preset
    this.spawnHatsForSession(sessionId, prompt);

    return { sessionId };
  }

  /**
   * Spawn a Hats process for a planning session.
   */
  private spawnHatsForSession(sessionId: string, prompt: string): void {
    const presetPath = path.join(this.workspaceRoot, "crates", "hats-cli", "presets", "planning.yml");

    const args = [
      "run",
      "-c", presetPath,
      "-p", prompt,  // Pass inline prompt text via -p flag (lowercase)
      "--no-tui",    // Disable TUI for background execution
    ];

    console.log(`[PlanningService] Spawning hats for session ${sessionId}:`, this.hatsPath, args.join(" "));

    const hatsProcess = spawn(this.hatsPath, args, {
      cwd: this.workspaceRoot,
      stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...process.env,
        HATS_PLANNING_SESSION_ID: sessionId,
      },
    });

    // Track the process
    this.runningProcesses.set(sessionId, hatsProcess);

    // Initialize processed events tracking for this session
    this.processedEventTimestamps.set(sessionId, new Set());

    // Start polling the events file for user.prompt events
    this.startEventPolling(sessionId);

    // Handle stdout (for logging only now, events come from JSONL file)
    hatsProcess.stdout?.on("data", (data: Buffer) => {
      const output = data.toString();
      console.log(`[PlanningService:${sessionId}] stdout:`, output.trim());
    });

    // Handle stderr
    hatsProcess.stderr?.on("data", (data: Buffer) => {
      console.error(`[PlanningService:${sessionId}] hats stderr:`, data.toString());
    });

    // Handle process exit
    hatsProcess.on("exit", (code, signal) => {
      console.log(`[PlanningService:${sessionId}] hats exited: code=${code}, signal=${signal}`);
      this.runningProcesses.delete(sessionId);
      this.stopEventPolling(sessionId);

      // Mark session as completed or failed
      this.updateSessionStatusOnExit(sessionId, code);
    });

    // Handle process error
    hatsProcess.on("error", (err) => {
      console.error(`[PlanningService:${sessionId}] hats error:`, err);
      this.runningProcesses.delete(sessionId);
      this.stopEventPolling(sessionId);
    });
  }

  /**
   * Start polling the Hats events file for user.prompt events.
   */
  private startEventPolling(sessionId: string): void {
    const pollIntervalMs = 500; // Poll every 500ms

    const poller = setInterval(() => {
      this.pollEventsFile(sessionId);
    }, pollIntervalMs);

    this.eventPollers.set(sessionId, poller);
    console.log(`[PlanningService:${sessionId}] Started event polling`);
  }

  /**
   * Stop polling the Hats events file.
   */
  private stopEventPolling(sessionId: string): void {
    const poller = this.eventPollers.get(sessionId);
    if (poller) {
      clearInterval(poller);
      this.eventPollers.delete(sessionId);
      this.processedEventTimestamps.delete(sessionId);
      console.log(`[PlanningService:${sessionId}] Stopped event polling`);
    }
  }

  /**
   * Get the current events file path from .hats/current-events.
   */
  private async getCurrentEventsPath(): Promise<string | null> {
    const currentEventsPath = path.join(this.workspaceRoot, ".hats", "current-events");
    try {
      const relativePath = await fs.readFile(currentEventsPath, "utf-8");
      return path.join(this.workspaceRoot, relativePath.trim());
    } catch (err) {
      console.warn("[PlanningService] Could not read current-events file:", err);
      return null;
    }
  }

  /**
   * Poll the events file for new user.prompt events.
   */
  private async pollEventsFile(sessionId: string): Promise<void> {
    const eventsPath = await this.getCurrentEventsPath();
    if (!eventsPath) {
      return;
    }

    try {
      const content = await fs.readFile(eventsPath, "utf-8");
      const lines = content.trim().split("\n").filter((l) => l.trim());
      const processedTimestamps = this.processedEventTimestamps.get(sessionId) ?? new Set<string>();

      for (const line of lines) {
        try {
          const event = JSON.parse(line) as HatsEvent;

          // Skip if we've already processed this event
          if (processedTimestamps.has(event.ts)) {
            continue;
          }

          // Process user.prompt events
          if (event.topic === "user.prompt") {
            const payload = event.payload as UserPromptPayload;
            const promptId = payload.id ?? `q${processedTimestamps.size + 1}`;
            const questionText = payload.question ?? (typeof payload === "string" ? payload : JSON.stringify(payload));

            console.log(`[PlanningService:${sessionId}] Detected user.prompt from events file: id=${promptId}`);

            // Append the prompt to the conversation file
            const conversationPath = path.join(this.sessionsDir, sessionId, "conversation.jsonl");
            const entry: ConversationEntry = {
              type: "user_prompt",
              id: promptId,
              text: questionText,
              ts: event.ts,
            };

            await fs.appendFile(conversationPath, JSON.stringify(entry) + "\n");

            // Update session status to waiting_for_input
            await this.updateSessionStatus(sessionId, SessionStatus.WaitingForInput);
          }

          // Mark this event as processed
          processedTimestamps.add(event.ts);
        } catch (parseErr) {
          console.warn(`[PlanningService:${sessionId}] Skipping malformed event line:`, parseErr);
        }
      }

      this.processedEventTimestamps.set(sessionId, processedTimestamps);
    } catch (err) {
      console.warn(`[PlanningService:${sessionId}] Could not read events file:`, err);
    }
  }

  /**
   * Update session status.
   */
  private async updateSessionStatus(sessionId: string, status: SessionStatus): Promise<void> {
    const metadataPath = path.join(this.sessionsDir, sessionId, "session.json");
    try {
      const content = await fs.readFile(metadataPath, "utf-8");
      const metadata: SessionMetadata = JSON.parse(content);
      metadata.status = status;
      metadata.updated_at = new Date().toISOString();
      await fs.writeFile(metadataPath, JSON.stringify(metadata, null, 2));
    } catch (err) {
      console.error(`[PlanningService:${sessionId}] Failed to update status:`, err);
    }
  }

  /**
   * Update session status when Hats exits.
   */
  private async updateSessionStatusOnExit(sessionId: string, exitCode: number | null): Promise<void> {
    const newStatus = exitCode === 0 ? SessionStatus.Completed : SessionStatus.Failed;
    await this.updateSessionStatus(sessionId, newStatus);
  }

  /**
   * Submit a user response to a planning session.
   */
  async submitResponse(
    sessionId: string,
    promptId: string,
    response: string
  ): Promise<void> {
    const conversationPath = path.join(this.sessionsDir, sessionId, "conversation.jsonl");

    // Create response entry
    const entry: ConversationEntry = {
      type: "user_response",
      id: promptId,
      text: response,
      ts: new Date().toISOString(),
    };

    // Append to conversation file
    await fs.appendFile(conversationPath, JSON.stringify(entry) + "\n");

    // Update session metadata (updated_at timestamp and status back to active)
    const sessionDir = path.join(this.sessionsDir, sessionId);
    const metadataPath = path.join(sessionDir, "session.json");
    const metadataContent = await fs.readFile(metadataPath, "utf-8");
    const metadata: SessionMetadata = JSON.parse(metadataContent);
    metadata.updated_at = new Date().toISOString();
    metadata.status = SessionStatus.Active;
    await fs.writeFile(metadataPath, JSON.stringify(metadata, null, 2));
  }

  /**
   * Delete a planning session.
   */
  async deleteSession(sessionId: string): Promise<void> {
    const sessionDir = path.join(this.sessionsDir, sessionId);

    // Kill any running process
    const process = this.runningProcesses.get(sessionId);
    if (process) {
      process.kill("SIGTERM");
      this.runningProcesses.delete(sessionId);
    }

    // Remove session directory recursively
    await fs.rm(sessionDir, { recursive: true, force: true });
  }

  /**
   * Resume a paused planning session.
   */
  async resumeSession(sessionId: string): Promise<void> {
    const sessionDir = path.join(this.sessionsDir, sessionId);

    // Check if session exists
    try {
      await fs.access(sessionDir);
    } catch {
      throw new Error(`Session ${sessionId} not found`);
    }

    // Load the prompt from metadata
    const metadataPath = path.join(sessionDir, "session.json");
    const metadataContent = await fs.readFile(metadataPath, "utf-8");
    const metadata: SessionMetadata = JSON.parse(metadataContent);

    // Update status to active
    metadata.status = SessionStatus.Active;
    metadata.updated_at = new Date().toISOString();
    await fs.writeFile(metadataPath, JSON.stringify(metadata, null, 2));

    // Spawn hats process if not already running
    if (!this.runningProcesses.has(sessionId)) {
      this.spawnHatsForSession(sessionId, metadata.prompt);
    }
  }

  /**
   * Ensure the planning sessions directory exists.
   */
  private async ensureSessionsDir(): Promise<void> {
    try {
      await fs.mkdir(this.sessionsDir, { recursive: true });
    } catch (err) {
      console.warn("[PlanningService] Failed to create sessions directory:", err);
    }
  }

  /**
   * Generate a unique session ID.
   */
  private generateSessionId(): string {
    const now = new Date();
    const timestamp = now
      .toISOString()
      .replace(/[-:.]/g, "")
      .slice(0, 15); // YYYYMMDDTHHmmss
    const random = uuidv4().slice(0, 8);
    return `${timestamp}-${random}`;
  }

  /**
   * Get the conversation file path for a session.
   */
  getConversationPath(sessionId: string): string {
    return path.join(this.sessionsDir, sessionId, "conversation.jsonl");
  }

  /**
   * Get the session directory path.
   */
  getSessionDir(sessionId: string): string {
    return path.join(this.sessionsDir, sessionId);
  }

  /**
   * Stop a running planning session.
   */
  async stopSession(sessionId: string): Promise<void> {
    const process = this.runningProcesses.get(sessionId);
    if (process) {
      process.kill("SIGTERM");
      this.runningProcesses.delete(sessionId);
      await this.updateSessionStatus(sessionId, SessionStatus.Paused);
    }
  }

  /**
   * Get artifact content for a specific session.
   * Returns the content of the artifact file.
   */
  async getArtifact(
    sessionId: string,
    filename: string
  ): Promise<{ content: string; filename: string }> {
    const sessionDir = path.join(this.sessionsDir, sessionId);
    const artifactsDir = path.join(sessionDir, "artifacts");
    const artifactPath = path.join(artifactsDir, filename);

    // Security: ensure the artifact path is within the session's artifacts directory
    const normalizedPath = path.normalize(artifactPath);
    if (!normalizedPath.startsWith(artifactsDir)) {
      throw new Error("Invalid artifact path");
    }

    try {
      const content = await fs.readFile(artifactPath, "utf-8");
      return { content, filename };
    } catch (err) {
      console.warn(`[PlanningService] Could not read artifact ${filename}:`, err);
      throw new Error(`Artifact not found: ${filename}`);
    }
  }
}
