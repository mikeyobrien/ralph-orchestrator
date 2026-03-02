import { useMutation, useQuery, useQueryClient, type QueryKey, type UseMutationOptions, type UseQueryOptions } from "@tanstack/react-query";
import { RpcClientError, rpcCall } from "./rpc/client";

type QueryOptions<TResult = any> = Omit<UseQueryOptions<TResult, RpcClientError>, "queryKey" | "queryFn">;
type MutationOptions<TResult = any, TInput = any> = Omit<UseMutationOptions<TResult, RpcClientError, TInput>, "mutationFn">;

interface QueryProcedure<TInput, TResult = any> {
  useQuery: (input?: TInput, options?: QueryOptions<TResult>) => any;
}

interface MutationProcedure<TInput, TResult = any> {
  useMutation: (options?: MutationOptions<TResult, TInput>) => any;
}

const QUERY_NAMESPACE = "rpc-v1";

function keyFor(scope: string, method: string, input?: unknown): QueryKey {
  return [QUERY_NAMESPACE, scope, method, input ?? null] as const;
}

function prefixFor(scope: string, method: string): QueryKey {
  return [QUERY_NAMESPACE, scope, method] as const;
}

function createQueryProcedure<TInput, TRpcResult, TResult>(config: {
  scope: string;
  method: string;
  mapInput?: (input?: TInput) => unknown;
  mapResult?: (result: TRpcResult, input?: TInput) => TResult;
}): QueryProcedure<TInput, TResult> {
  return {
    useQuery: (input?: TInput, options?: QueryOptions<TResult>) =>
      useQuery<TResult, RpcClientError>({
        queryKey: keyFor(config.scope, config.method, input),
        queryFn: async () => {
          const params = config.mapInput ? config.mapInput(input) : ((input as unknown) ?? {});
          const result = await rpcCall<TRpcResult>(config.method, params, { mutating: false });
          if (config.mapResult) {
            return config.mapResult(result, input);
          }
          return result as unknown as TResult;
        },
        ...(options ?? {}),
      }),
  };
}

function createMutationProcedure<TInput, TRpcResult, TResult>(config: {
  method: string;
  mapInput?: (input: TInput) => unknown;
  mapResult?: (result: TRpcResult, input: TInput) => TResult;
}): MutationProcedure<TInput, TResult> {
  return {
    useMutation: (options?: MutationOptions<TResult, TInput>) =>
      useMutation<TResult, RpcClientError, TInput>({
        mutationFn: async (input: TInput) => {
          const params = config.mapInput ? config.mapInput(input) : (input as unknown);
          const result = await rpcCall<TRpcResult>(config.method, params);
          if (config.mapResult) {
            return config.mapResult(result, input);
          }
          return result as unknown as TResult;
        },
        ...(options ?? {}),
      }),
  };
}

async function listLoopsWithMergeState(input?: { includeTerminal?: boolean }) {
  const result = await rpcCall<{ loops: Array<Record<string, unknown>> }>("loop.list", input ?? {}, {
    mutating: false,
  });

  const loops = result.loops ?? [];
  const enriched = await Promise.all(
    loops.map(async (loop) => {
      const location = typeof loop.location === "string" ? loop.location : "";
      const id = typeof loop.id === "string" ? loop.id : undefined;

      if (!id || location === "(in-place)") {
        return loop;
      }

      try {
        const mergeState = await rpcCall<{ enabled: boolean; reason?: string }>(
          "loop.merge_button_state",
          { id },
          { mutating: false }
        );

        return {
          ...loop,
          mergeButtonState: {
            state: mergeState.enabled ? "active" : "blocked",
            reason: mergeState.reason,
          },
        };
      } catch {
        return loop;
      }
    })
  );

  return enriched;
}

function useRpcUtils() {
  const queryClient = useQueryClient();

  const invalidatePrefix = (scope: string, method: string) =>
    queryClient.invalidateQueries({ queryKey: prefixFor(scope, method) });

  const invalidateExact = (scope: string, method: string, input?: unknown) =>
    queryClient.invalidateQueries({
      queryKey: keyFor(scope, method, input),
      exact: true,
    });

  return {
    task: {
      list: {
        invalidate: (input?: unknown) =>
          input === undefined
            ? invalidatePrefix("task", "task.list")
            : invalidateExact("task", "task.list", input),
      },
      ready: {
        invalidate: () => invalidatePrefix("task", "task.ready"),
      },
      get: {
        invalidate: (input?: unknown) =>
          input === undefined
            ? invalidatePrefix("task", "task.get")
            : invalidateExact("task", "task.get", input),
      },
    },
    loops: {
      list: {
        invalidate: (input?: unknown) =>
          input === undefined
            ? invalidatePrefix("loop", "loop.list")
            : invalidateExact("loop", "loop.list", input),
      },
      managerStatus: {
        invalidate: () => invalidatePrefix("loop", "loop.status"),
      },
    },
    planning: {
      list: {
        invalidate: () => invalidatePrefix("planning", "planning.list"),
      },
      get: {
        invalidate: (input?: unknown) =>
          input === undefined
            ? invalidatePrefix("planning", "planning.get")
            : invalidateExact("planning", "planning.get", input),
      },
    },
    config: {
      get: {
        invalidate: () => invalidatePrefix("config", "config.get"),
      },
    },
    presets: {
      list: {
        invalidate: () => invalidatePrefix("preset", "preset.list"),
      },
    },
    collection: {
      list: {
        invalidate: () => invalidatePrefix("collection", "collection.list"),
      },
      get: {
        invalidate: (input?: unknown) =>
          input === undefined
            ? invalidatePrefix("collection", "collection.get")
            : invalidateExact("collection", "collection.get", input),
      },
    },
  };
}

export const trpc = {
  useUtils: useRpcUtils,

  task: {
    list: createQueryProcedure<{ status?: string; includeArchived?: boolean }, { tasks: any[] }, any[]>({
      scope: "task",
      method: "task.list",
      mapInput: (input) => input ?? {},
      mapResult: (result) => result.tasks ?? [],
    }),

    get: createQueryProcedure<{ id: string }, { task: any }, any>({
      scope: "task",
      method: "task.get",
      mapInput: (input) => input ?? {},
      mapResult: (result) => result.task,
    }),

    ready: createQueryProcedure<void, { tasks: any[] }, any[]>({
      scope: "task",
      method: "task.ready",
      mapInput: () => ({}),
      mapResult: (result) => result.tasks ?? [],
    }),

    create: createMutationProcedure<
      {
        id: string;
        title: string;
        status?: string;
        priority?: number;
        blockedBy?: string | null;
        autoExecute?: boolean;
        preset?: string;
      },
      { task: any },
      any
    >({
      method: "task.create",
      mapInput: (input) => {
        const { preset: _preset, ...rest } = input;
        return rest;
      },
      mapResult: (result) => result.task,
    }),

    run: createMutationProcedure<{ id: string }, { success: boolean; queuedTaskId?: string; task?: any }, {
      success: boolean;
      queuedTaskId?: string;
      task?: any;
    }>({
      method: "task.run",
    }),

    runAll: createMutationProcedure<void, { enqueued: number; errors: string[] }, { enqueued: number; errors: string[] }>({
      method: "task.run_all",
      mapInput: () => ({}),
    }),

    retry: createMutationProcedure<{ id: string }, { success: boolean; queuedTaskId?: string; task?: any }, {
      success: boolean;
      queuedTaskId?: string;
      task?: any;
    }>({
      method: "task.retry",
    }),

    executionStatus: createQueryProcedure<{ id: string }, { isQueued: boolean; queuePosition?: number; runnerPid?: number }, {
      isQueued: boolean;
      queuePosition?: number;
      runnerPid?: number;
    }>({
      scope: "task",
      method: "task.status",
    }),

    cancel: createMutationProcedure<{ id: string }, { task: any }, { success: boolean; task: any }>({
      method: "task.cancel",
      mapResult: (result) => ({ success: true, task: result.task }),
    }),

    update: createMutationProcedure<
      { id: string; title?: string; status?: string; priority?: number; blockedBy?: string | null },
      { task: any },
      any
    >({
      method: "task.update",
      mapResult: (result) => result.task,
    }),

    close: createMutationProcedure<{ id: string }, { task: any }, any>({
      method: "task.close",
      mapResult: (result) => result.task,
    }),

    archive: createMutationProcedure<{ id: string }, { task: any }, any>({
      method: "task.archive",
      mapResult: (result) => result.task,
    }),

    unarchive: createMutationProcedure<{ id: string }, { task: any }, any>({
      method: "task.unarchive",
      mapResult: (result) => result.task,
    }),

    delete: createMutationProcedure<{ id: string }, { success: boolean }, { success: boolean }>({
      method: "task.delete",
    }),

    clearAll: createMutationProcedure<void, { success: boolean }, { success: boolean; deletedTasks: number; deletedLogs: number }>({
      method: "task.clear",
      mapInput: () => ({}),
      mapResult: (result) => ({
        success: Boolean(result.success),
        deletedTasks: 0,
        deletedLogs: 0,
      }),
    }),
  },

  loops: {
    list: {
      useQuery: ((input?: { includeTerminal?: boolean }, options?: QueryOptions<any[]>) =>
        useQuery<any[], RpcClientError>({
          queryKey: keyFor("loop", "loop.list", input),
          queryFn: () => listLoopsWithMergeState(input),
          ...(options ?? {}),
        })) as any,
    },

    managerStatus: createQueryProcedure<void, { running: boolean; intervalMs: number; lastProcessedAt?: string }, {
      running: boolean;
      intervalMs: number;
      lastProcessedAt?: string;
    }>({
      scope: "loop",
      method: "loop.status",
      mapInput: () => ({}),
    }),

    process: createMutationProcedure<void, { success: boolean }, { success: boolean }>({
      method: "loop.process",
      mapInput: () => ({}),
    }),

    prune: createMutationProcedure<void, { success: boolean }, { success: boolean }>({
      method: "loop.prune",
      mapInput: () => ({}),
    }),

    retry: createMutationProcedure<{ id: string; steeringInput?: string }, { success: boolean }, { success: boolean }>({
      method: "loop.retry",
    }),

    discard: createMutationProcedure<{ id: string }, { success: boolean }, { success: boolean }>({
      method: "loop.discard",
    }),

    stop: createMutationProcedure<{ id: string; force?: boolean }, { success: boolean }, { success: boolean }>({
      method: "loop.stop",
    }),

    merge: createMutationProcedure<{ id: string; force?: boolean }, { success: boolean }, { success: boolean }>({
      method: "loop.merge",
    }),

    triggerMergeTask: createMutationProcedure<{ loopId: string }, { success: boolean; taskId: string; queuedTaskId?: string }, {
      success: boolean;
      taskId: string;
      queuedTaskId?: string;
    }>({
      method: "loop.trigger_merge_task",
    }),

    mergeButtonState: createQueryProcedure<{ id: string }, { enabled: boolean; reason?: string }, {
      state: "active" | "blocked";
      reason?: string;
    }>({
      scope: "loop",
      method: "loop.merge_button_state",
      mapResult: (result) => ({
        state: result.enabled ? "active" : "blocked",
        reason: result.reason,
      }),
    }),
  },

  planning: {
    list: createQueryProcedure<void, { sessions: any[] }, any[]>({
      scope: "planning",
      method: "planning.list",
      mapInput: () => ({}),
      mapResult: (result) => result.sessions ?? [],
    }),

    get: createQueryProcedure<{ id: string }, { session: any }, any>({
      scope: "planning",
      method: "planning.get",
      mapResult: (result) => result.session,
    }),

    start: createMutationProcedure<{ prompt: string }, { session: { id: string } }, { sessionId: string }>({
      method: "planning.start",
      mapResult: (result) => ({ sessionId: result.session.id }),
    }),

    respond: createMutationProcedure<
      { sessionId: string; promptId: string; response: string },
      { success: boolean },
      { success: boolean }
    >({
      method: "planning.respond",
    }),

    resume: createMutationProcedure<{ id: string }, { success: boolean }, { success: boolean }>({
      method: "planning.resume",
    }),

    delete: createMutationProcedure<{ id: string }, { success: boolean }, { success: boolean }>({
      method: "planning.delete",
    }),

    getArtifact: createQueryProcedure<{ sessionId: string; filename: string }, { filename: string; content: string }, {
      filename: string;
      content: string;
    }>({
      scope: "planning",
      method: "planning.get_artifact",
    }),
  },

  config: {
    get: createQueryProcedure<void, { raw: string; parsed: Record<string, unknown> }, { raw: string; parsed: Record<string, unknown> }>({
      scope: "config",
      method: "config.get",
      mapInput: () => ({}),
    }),

    update: createMutationProcedure<{ content: string }, { success: boolean; parsed: Record<string, unknown> }, {
      success: boolean;
      parsed: Record<string, unknown>;
    }>({
      method: "config.update",
    }),
  },

  presets: {
    list: createQueryProcedure<void, { presets: any[] }, any[]>({
      scope: "preset",
      method: "preset.list",
      mapInput: () => ({}),
      mapResult: (result) => result.presets ?? [],
    }),
  },

  collection: {
    list: createQueryProcedure<void, { collections: any[] }, any[]>({
      scope: "collection",
      method: "collection.list",
      mapInput: () => ({}),
      mapResult: (result) => result.collections ?? [],
    }),

    get: createQueryProcedure<{ id: string }, { collection: any }, any>({
      scope: "collection",
      method: "collection.get",
      mapResult: (result) => result.collection,
    }),

    create: createMutationProcedure<{ name: string; description?: string; graph?: any }, { collection: any }, any>({
      method: "collection.create",
      mapResult: (result) => result.collection,
    }),

    update: createMutationProcedure<{ id: string; name?: string; description?: string; graph?: any }, { collection: any }, any>({
      method: "collection.update",
      mapResult: (result) => result.collection,
    }),

    delete: createMutationProcedure<{ id: string }, { success: boolean }, { success: boolean }>({
      method: "collection.delete",
    }),

    exportYaml: createQueryProcedure<{ id: string }, { yaml: string }, { yaml: string }>({
      scope: "collection",
      method: "collection.export",
    }),

    importYaml: createMutationProcedure<{ yaml: string; name: string; description?: string }, { collection: any }, any>({
      method: "collection.import",
      mapResult: (result) => result.collection,
    }),
  },
};

export function createTRPCClient() {
  return {
    kind: "rpc-v1-client",
  };
}
