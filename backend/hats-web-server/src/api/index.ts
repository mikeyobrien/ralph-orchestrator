/**
 * API Module Exports
 *
 * Barrel exports for the Fastify/TRPC API layer.
 */

// Server exports
export { createServer, startServer } from "./server";
export type { ServerOptions } from "./server";

// TRPC exports
export { appRouter, taskRouter, router, publicProcedure, createContext } from "./trpc";
export type { AppRouter, Context } from "./trpc";

// REST API exports
export { registerRestRoutes } from "./rest";

// WebSocket log streaming exports
export {
  LogBroadcaster,
  getLogBroadcaster,
  configureLogBroadcaster,
  resetLogBroadcaster,
} from "./LogBroadcaster";
export type { LogMessage, LogBroadcasterOptions } from "./LogBroadcaster";
