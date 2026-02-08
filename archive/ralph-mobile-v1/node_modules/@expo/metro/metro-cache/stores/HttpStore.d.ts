import HttpError from "./HttpError";
import NetworkError from "./NetworkError";
export type Options = EndpointOptions | {
  getOptions: EndpointOptions;
  setOptions: EndpointOptions;
};
export interface _EndpointOptions_headers {
  [$$Key$$: string]: string;
}
export interface EndpointOptions {
  endpoint: string;
  family?: 4 | 6;
  timeout?: number;
  key?: string | ReadonlyArray<string> | Buffer | ReadonlyArray<Buffer>;
  cert?: string | ReadonlyArray<string> | Buffer | ReadonlyArray<Buffer>;
  ca?: string | ReadonlyArray<string> | Buffer | ReadonlyArray<Buffer>;
  params?: URLSearchParams;
  headers?: _EndpointOptions_headers;
  additionalSuccessStatuses?: ReadonlyArray<number>;
  /**
   * Whether to include additional debug information in error messages.
   */
  debug?: boolean;
  /**
   * Retry configuration
   */
  maxAttempts?: number;
  retryNetworkErrors?: boolean;
  retryStatuses?: ReadonlySet<number>;
  socketPath?: string;
  proxy?: string;
}
declare class HttpStore<T> {
  static HttpError: typeof HttpError;
  static NetworkError: typeof NetworkError;
  constructor(options: Options);
  get(key: Buffer): Promise<null | undefined | T>;
  set(key: Buffer, value: T): Promise<void>;
  clear(): void;
}
export default HttpStore;