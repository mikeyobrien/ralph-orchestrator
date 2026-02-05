/**
 * REST API Router
 *
 * Provides standard REST endpoints at /api/v1/* alongside the existing tRPC API.
 * These endpoints expose task, hat, and preset functionality for external consumers
 * that cannot use a tRPC client.
 *
 * Endpoints:
 *   GET    /api/v1/health          - Health check with version info
 *   GET    /api/v1/tasks           - List all tasks
 *   POST   /api/v1/tasks           - Create a new task
 *   GET    /api/v1/tasks/:id       - Get task by ID
 *   PATCH  /api/v1/tasks/:id       - Update a task
 *   DELETE /api/v1/tasks/:id       - Delete a task
 *   POST   /api/v1/tasks/:id/run   - Execute a task
 *   GET    /api/v1/hats            - List all hats
 *   GET    /api/v1/hats/:key       - Get hat by key
 *   GET    /api/v1/presets         - List all presets
 */

import { FastifyInstance } from "fastify";
import { Context, getBuiltinPresets, getDirectoryPresets } from "./trpc";

/**
 * Register all REST API routes on the Fastify instance.
 *
 * @param server - Fastify server instance
 * @param ctx - Shared context with repositories and services
 */
export async function registerRestRoutes(
  server: FastifyInstance,
  ctx: Context
): Promise<void> {
  // 1. GET /api/v1/health - Health check with version info
  server.get("/api/v1/health", async (_request, reply) => {
    return reply.send({
      status: "ok",
      version: "1.0.0",
      timestamp: new Date().toISOString(),
    });
  });

  // 2. GET /api/v1/tasks - List all tasks
  server.get<{
    Querystring: { status?: string; includeArchived?: string };
  }>("/api/v1/tasks", async (request, reply) => {
    const { status, includeArchived } = request.query;
    const tasks = ctx.taskRepository.findAll(
      status,
      includeArchived === "true"
    );
    return reply.send(tasks);
  });

  // 3. POST /api/v1/tasks - Create a new task
  server.post<{
    Body: {
      id: string;
      title: string;
      status?: string;
      priority?: number;
      blockedBy?: string | null;
      autoExecute?: boolean;
      preset?: string;
    };
  }>("/api/v1/tasks", async (request, reply) => {
    const { id, title, status, priority, blockedBy, autoExecute, preset } =
      request.body ?? {};

    if (!id || !title) {
      return reply.status(400).send({
        error: "Bad Request",
        message: "id and title are required",
      });
    }

    if (priority !== undefined && (priority < 1 || priority > 5)) {
      return reply.status(400).send({
        error: "Bad Request",
        message: "priority must be between 1 and 5",
      });
    }

    const task = ctx.taskRepository.create({
      id,
      title,
      status: status ?? "open",
      priority: priority ?? 2,
      blockedBy: blockedBy ?? undefined,
    });

    // Auto-execute if requested and bridge is available
    if (autoExecute !== false && ctx.taskBridge && !task.blockedBy) {
      ctx.taskBridge.enqueueTask(task, preset);
      const updated = ctx.taskRepository.findById(task.id);
      return reply.status(201).send(updated ?? task);
    }

    return reply.status(201).send(task);
  });

  // 4. GET /api/v1/tasks/:id - Get task by ID
  server.get<{ Params: { id: string } }>(
    "/api/v1/tasks/:id",
    async (request, reply) => {
      const task = ctx.taskRepository.findById(request.params.id);
      if (!task) {
        return reply.status(404).send({
          error: "Not Found",
          message: `Task with id '${request.params.id}' not found`,
        });
      }
      return reply.send(task);
    }
  );

  // 5. PATCH /api/v1/tasks/:id - Update a task
  server.patch<{
    Params: { id: string };
    Body: {
      title?: string;
      status?: string;
      priority?: number;
      blockedBy?: string | null;
    };
  }>("/api/v1/tasks/:id", async (request, reply) => {
    const { title, status, priority, blockedBy } = request.body ?? {};

    if (priority !== undefined && (priority < 1 || priority > 5)) {
      return reply.status(400).send({
        error: "Bad Request",
        message: "priority must be between 1 and 5",
      });
    }

    if (title !== undefined && title.length === 0) {
      return reply.status(400).send({
        error: "Bad Request",
        message: "title must not be empty",
      });
    }

    const task = ctx.taskRepository.update(request.params.id, {
      title,
      status,
      priority,
      blockedBy,
    });

    if (!task) {
      return reply.status(404).send({
        error: "Not Found",
        message: `Task with id '${request.params.id}' not found`,
      });
    }

    return reply.send(task);
  });

  // 6. DELETE /api/v1/tasks/:id - Delete a task
  server.delete<{ Params: { id: string } }>(
    "/api/v1/tasks/:id",
    async (request, reply) => {
      const task = ctx.taskRepository.findById(request.params.id);
      if (!task) {
        return reply.status(404).send({
          error: "Not Found",
          message: `Task with id '${request.params.id}' not found`,
        });
      }

      const deletableStates = ["failed", "closed"];
      if (!deletableStates.includes(task.status)) {
        return reply.status(409).send({
          error: "Conflict",
          message: `Cannot delete task in '${task.status}' state. Only failed or closed tasks can be deleted.`,
        });
      }

      const deleted = ctx.taskRepository.delete(request.params.id);
      if (!deleted) {
        return reply.status(500).send({
          error: "Internal Server Error",
          message: `Failed to delete task '${request.params.id}'`,
        });
      }

      return reply.status(204).send();
    }
  );

  // 7. POST /api/v1/tasks/:id/run - Execute a task
  server.post<{ Params: { id: string } }>(
    "/api/v1/tasks/:id/run",
    async (request, reply) => {
      if (!ctx.taskBridge) {
        return reply.status(503).send({
          error: "Service Unavailable",
          message: "Task execution is not configured",
        });
      }

      const task = ctx.taskRepository.findById(request.params.id);
      if (!task) {
        return reply.status(404).send({
          error: "Not Found",
          message: `Task with id '${request.params.id}' not found`,
        });
      }

      const result = ctx.taskBridge.enqueueTask(task);
      if (!result.success) {
        return reply.status(400).send({
          error: "Bad Request",
          message: result.error || "Failed to enqueue task",
        });
      }

      return reply.send({
        success: true,
        queuedTaskId: result.queuedTaskId,
        task: ctx.taskRepository.findById(request.params.id),
      });
    }
  );

  // 8. GET /api/v1/hats - List all hats
  server.get("/api/v1/hats", async (_request, reply) => {
    const definitions = ctx.settingsService.getHatDefinitions();
    const activeHat = ctx.settingsService.getActiveHat();

    const hats = Object.entries(definitions).map(([key, hat]) => ({
      key,
      ...hat,
      isActive: key === activeHat,
    }));

    return reply.send(hats);
  });

  // 9. GET /api/v1/hats/:key - Get hat by key
  server.get<{ Params: { key: string } }>(
    "/api/v1/hats/:key",
    async (request, reply) => {
      const hat = ctx.settingsService.getHat(request.params.key);
      if (!hat) {
        return reply.status(404).send({
          error: "Not Found",
          message: `Hat '${request.params.key}' not found`,
        });
      }

      const activeKey = ctx.settingsService.getActiveHat();
      return reply.send({
        key: request.params.key,
        ...hat,
        isActive: request.params.key === activeKey,
      });
    }
  );

  // 10. GET /api/v1/presets - List all presets
  server.get("/api/v1/presets", async (_request, reply) => {
    const builtinPresets = getBuiltinPresets();
    const directoryPresets = getDirectoryPresets();

    const collections = ctx.collectionService.listCollections();
    const collectionPresets = collections.map((c) => ({
      id: c.id,
      name: c.name,
      source: "collection" as const,
      description: c.description ?? undefined,
    }));

    return reply.send([
      ...builtinPresets,
      ...directoryPresets,
      ...collectionPresets,
    ]);
  });
}
