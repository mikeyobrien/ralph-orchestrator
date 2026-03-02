const RPC_ENDPOINT = "/rpc/v1";

const MUTATING_METHODS = new Set<string>([
  "task.create",
  "task.update",
  "task.close",
  "task.archive",
  "task.unarchive",
  "task.delete",
  "task.clear",
  "task.run",
  "task.run_all",
  "task.retry",
  "task.cancel",
  "loop.process",
  "loop.prune",
  "loop.retry",
  "loop.discard",
  "loop.stop",
  "loop.merge",
  "loop.trigger_merge_task",
  "planning.start",
  "planning.respond",
  "planning.resume",
  "planning.delete",
  "config.update",
  "collection.create",
  "collection.update",
  "collection.delete",
  "collection.import",
]);

let requestCounter = 0;

interface RpcErrorBody {
  code: string;
  message: string;
  retryable: boolean;
  details?: unknown;
}

interface RpcResponseEnvelope<TResult> {
  apiVersion: string;
  id: string;
  method?: string;
  result?: TResult;
  error?: RpcErrorBody;
}

interface RpcCallOptions {
  mutating?: boolean;
  signal?: AbortSignal;
  endpoint?: string;
}

export class RpcClientError extends Error {
  code: string;
  retryable: boolean;
  details?: unknown;
  status?: number;

  constructor(message: string, options: { code?: string; retryable?: boolean; details?: unknown; status?: number } = {}) {
    super(message);
    this.name = "RpcClientError";
    this.code = options.code ?? "INTERNAL";
    this.retryable = options.retryable ?? false;
    this.details = options.details;
    this.status = options.status;
  }
}

function nextRequestId(): string {
  requestCounter = (requestCounter + 1) % Number.MAX_SAFE_INTEGER;
  return `req-${Date.now()}-${requestCounter.toString(16).padStart(4, "0")}`;
}

function nextIdempotencyKey(method: string): string {
  return `idem-${method.replace(/\./g, "-")}-${Date.now()}-${Math.random().toString(16).slice(2, 10)}`;
}

function parsePayloadError(payload: unknown, fallbackStatus: number): RpcClientError {
  if (payload && typeof payload === "object") {
    const objectPayload = payload as Record<string, unknown>;
    const envelopeError = objectPayload.error;

    if (envelopeError && typeof envelopeError === "object") {
      const error = envelopeError as Record<string, unknown>;
      return new RpcClientError(
        typeof error.message === "string" ? error.message : "RPC request failed",
        {
          code: typeof error.code === "string" ? error.code : "INTERNAL",
          retryable: Boolean(error.retryable),
          details: error.details,
          status: fallbackStatus,
        }
      );
    }

    if (typeof objectPayload.message === "string") {
      return new RpcClientError(objectPayload.message, { status: fallbackStatus });
    }
  }

  return new RpcClientError("RPC request failed", { status: fallbackStatus });
}

export async function rpcCall<TResult>(
  method: string,
  params: unknown = {},
  options: RpcCallOptions = {}
): Promise<TResult> {
  const requestId = nextRequestId();
  const isMutating = options.mutating ?? MUTATING_METHODS.has(method);
  const endpoint = options.endpoint ?? RPC_ENDPOINT;

  const body: Record<string, unknown> = {
    apiVersion: "v1",
    id: requestId,
    method,
    params: params ?? {},
  };

  if (isMutating) {
    body.meta = {
      idempotencyKey: nextIdempotencyKey(method),
      requestTs: new Date().toISOString(),
    };
  }

  let response: Response;
  try {
    response = await fetch(endpoint, {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify(body),
      signal: options.signal,
    });
  } catch (error) {
    if (error instanceof Error) {
      throw new RpcClientError(error.message, { code: "SERVICE_UNAVAILABLE", retryable: true });
    }
    throw new RpcClientError("Network request failed", {
      code: "SERVICE_UNAVAILABLE",
      retryable: true,
    });
  }

  let payload: unknown;
  try {
    payload = (await response.json()) as unknown;
  } catch {
    throw new RpcClientError("RPC response is not valid JSON", {
      code: "INTERNAL",
      status: response.status,
    });
  }

  const envelope = payload as RpcResponseEnvelope<TResult>;
  if (!response.ok || envelope.error) {
    throw parsePayloadError(payload, response.status);
  }

  if (!("result" in envelope)) {
    throw new RpcClientError("RPC response is missing result payload", {
      code: "INTERNAL",
      status: response.status,
    });
  }

  return envelope.result as TResult;
}

export interface StreamSubscribeParams {
  topics: string[];
  cursor?: string;
  replayLimit?: number;
  filters?: Record<string, unknown>;
}

export interface StreamSubscribeResult {
  subscriptionId: string;
  acceptedTopics: string[];
  cursor: string;
}

export interface StreamEventEnvelope {
  apiVersion: string;
  stream: string;
  topic: string;
  cursor: string;
  sequence: number;
  ts: string;
  resource: {
    type: string;
    id: string;
  };
  replay: {
    mode: "live" | "replay" | "resume";
    requestedCursor?: string;
    batch?: number;
  };
  payload: unknown;
}

export async function rpcSubscribe(params: StreamSubscribeParams): Promise<StreamSubscribeResult> {
  return rpcCall<StreamSubscribeResult>("stream.subscribe", params, { mutating: false });
}

export async function rpcUnsubscribe(subscriptionId: string): Promise<void> {
  await rpcCall<{ success: boolean }>("stream.unsubscribe", { subscriptionId }, { mutating: false });
}

export async function rpcAck(subscriptionId: string, cursor: string): Promise<void> {
  await rpcCall<{ success: boolean }>("stream.ack", { subscriptionId, cursor }, { mutating: false });
}

export function buildStreamWebSocketUrl(subscriptionId: string, wsUrl?: string): string {
  if (wsUrl) {
    try {
      const url = new URL(wsUrl, window.location.href);
      url.searchParams.set("subscriptionId", subscriptionId);
      return url.toString();
    } catch {
      const separator = wsUrl.includes("?") ? "&" : "?";
      return `${wsUrl}${separator}subscriptionId=${encodeURIComponent(subscriptionId)}`;
    }
  }

  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const host = window.location.host;
  return `${protocol}//${host}/rpc/v1/stream?subscriptionId=${encodeURIComponent(subscriptionId)}`;
}
