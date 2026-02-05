/**
 * Fastify Server with TRPC Integration and WebSocket Support
 *
 * HTTP server providing:
 * - /health endpoint for health checks
 * - /trpc/* endpoints for TRPC API
 * - /ws/logs WebSocket endpoint for real-time log streaming
 * - CORS support for cross-origin requests
 */

import Fastify, { FastifyInstance } from "fastify";
import cors from "@fastify/cors";
import websocket from "@fastify/websocket";
import { fastifyTRPCPlugin, FastifyTRPCPluginOptions } from "@trpc/server/adapters/fastify";
import { appRouter, createContext, AppRouter } from "./trpc";
import { getDatabase } from "../db/connection";
import { BetterSQLite3Database } from "drizzle-orm/better-sqlite3";
import * as schema from "../db/schema";
import { getLogBroadcaster } from "./LogBroadcaster";
import { registerRestRoutes } from "./rest";
import { TaskBridge } from "../services/TaskBridge";
import { LoopsManager } from "../services/LoopsManager";
import { PlanningService } from "../services/PlanningService";

export interface ServerOptions {
  /** Port to listen on (default: 3000) */
  port?: number;
  /** Host to bind to (default: '0.0.0.0') */
  host?: string;
  /** Optional database instance (creates one if not provided) */
  db?: BetterSQLite3Database<typeof schema>;
  /** Enable request logging (default: true) */
  logger?: boolean;
  /** TaskBridge for task execution (optional) */
  taskBridge?: TaskBridge;
  /** LoopsManager for loop operations (optional) */
  loopsManager?: LoopsManager;
  /** PlanningService for planning sessions (optional) */
  planningService?: PlanningService;
}

/**
 * Create and configure a Fastify server with TRPC
 */
export async function createServer(options: ServerOptions = {}): Promise<FastifyInstance> {
  const { port = 3000, host = "0.0.0.0", db = getDatabase(), logger = true, taskBridge, loopsManager, planningService } = options;

  const server = Fastify({ logger });

  // Register CORS
  await server.register(cors, {
    origin: true, // Allow all origins in development
    methods: ["GET", "POST", "PATCH", "DELETE", "OPTIONS"],
    credentials: true,
  });

  // Register WebSocket plugin
  await server.register(websocket);

  // Health check endpoint
  server.get("/health", async () => {
    return { status: "ok", timestamp: new Date().toISOString() };
  });

  // WebSocket endpoint for log streaming
  server.get("/ws/logs", { websocket: true }, (socket, _req) => {
    const broadcaster = getLogBroadcaster();
    const clientId = broadcaster.addClient(socket);

    // Handle incoming messages from client
    socket.on("message", (rawMessage: Buffer | string) => {
      try {
        const message = JSON.parse(rawMessage.toString());

        if (message.type === "subscribe" && message.taskId) {
          const sinceId = typeof message.sinceId === "number" ? message.sinceId : undefined;
          broadcaster.subscribe(clientId, message.taskId, { sinceId });
        } else if (message.type === "unsubscribe" && message.taskId) {
          broadcaster.unsubscribe(clientId, message.taskId);
        }
      } catch (err) {
        console.warn(`[WebSocket] Invalid message from client ${clientId}:`, err);
        socket.send(
          JSON.stringify({
            type: "error",
            taskId: "",
            data: { error: "Invalid message format. Expected JSON with type and taskId." },
            timestamp: new Date().toISOString(),
          })
        );
      }
    });

    // Send welcome message
    socket.send(
      JSON.stringify({
        type: "status",
        taskId: "",
        data: { status: "connected", clientId },
        timestamp: new Date().toISOString(),
      })
    );
  });

  // Register TRPC plugin
  await server.register(fastifyTRPCPlugin, {
    prefix: "/trpc",
    trpcOptions: {
      router: appRouter,
      createContext: () => createContext(db, taskBridge, loopsManager, planningService),
      onError: ({ path, error }) => {
        console.error(`TRPC Error on ${path}:`, error);
      },
    } satisfies FastifyTRPCPluginOptions<AppRouter>["trpcOptions"],
  });

  // Register REST API routes at /api/v1/*
  const ctx = createContext(db, taskBridge, loopsManager, planningService);
  await registerRestRoutes(server, ctx);

  return server;
}

/**
 * Start the server and listen on the specified port
 */
export async function startServer(options: ServerOptions = {}): Promise<FastifyInstance> {
  const { port = 3000, host = "0.0.0.0" } = options;

  const server = await createServer(options);

  try {
    const address = await server.listen({ port, host });
    console.log(`Server listening at ${address}`);
    return server;
  } catch (err) {
    server.log.error(err);
    throw err;
  }
}

// Export for direct CLI usage
export { appRouter, AppRouter } from "./trpc";
