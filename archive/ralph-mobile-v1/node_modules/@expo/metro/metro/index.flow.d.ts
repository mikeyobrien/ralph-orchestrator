import type * as _ws from "ws";
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow
 * @format
 * @oncall react_native
 */

import type { AssetData } from "./Assets";
import type { ReadOnlyGraph } from "./DeltaBundler";
import type { ServerOptions } from "./Server";
import type { BuildOptions, OutputOptions, RequestOptions } from "./shared/types";
import type { HandleFunction } from "connect";
import type { Server as HttpServer } from "node:http";
import type { Server as HttpsServer } from "node:https";
import type { TransformProfile } from "../metro-babel-transformer";
import type { ConfigT, InputConfigT, MetroConfig, Middleware } from "../metro-config";
import type { CustomResolverOptions } from "../metro-resolver";
import type { CustomTransformOptions } from "../metro-transform-worker";
import type $$IMPORT_TYPEOF_1$$ from "yargs";
type Yargs = typeof $$IMPORT_TYPEOF_1$$;
import JsonReporter from "./lib/JsonReporter";
import TerminalReporter from "./lib/TerminalReporter";
import MetroServer from "./Server";
import { loadConfig, mergeConfig, resolveConfig } from "../metro-config";
import { Terminal } from "../metro-core";
export interface MetroMiddleWare {
  attachHmrServer: (httpServer: HttpServer | HttpsServer) => void;
  end: () => Promise<void>;
  metroServer: MetroServer;
  middleware: Middleware;
}
export interface RunMetroOptions extends ServerOptions {
  waitForBundler?: boolean;
}
export interface _RunServerOptions_websocketEndpoints {
  readonly [path: string]: _ws.WebSocketServer;
}
export interface RunServerOptions {
  readonly hasReducedPerformance?: boolean;
  readonly host?: string;
  readonly onError?: ($$PARAM_0$$: Error & {
    code?: string;
  }) => void;
  readonly onReady?: (server: HttpServer | HttpsServer) => void;
  readonly onClose?: () => void;
  readonly secureServerOptions?: Object;
  readonly secure?: boolean;
  readonly secureCert?: string;
  readonly secureKey?: string;
  readonly unstable_extraMiddleware?: ReadonlyArray<HandleFunction>;
  readonly waitForBundler?: boolean;
  readonly watch?: boolean;
  readonly websocketEndpoints?: _RunServerOptions_websocketEndpoints;
}
export interface RunServerResult {
  httpServer?: HttpServer | HttpsServer;
}
export interface BuildGraphOptions {
  entries: ReadonlyArray<string>;
  customTransformOptions?: CustomTransformOptions;
  dev?: boolean;
  minify?: boolean;
  onProgress?: (transformedFileCount: number, totalFileCount: number) => void;
  platform?: string;
  type?: "module" | "script";
}
export interface _RunBuildOptions_output {
  readonly build: ($$PARAM_0$$: MetroServer, $$PARAM_1$$: RequestOptions, $$PARAM_2$$: void | BuildOptions) => Promise<{
    code: string;
    map: string;
    assets?: ReadonlyArray<AssetData>;
  }>;
  readonly save: ($$PARAM_0$$: {
    code: string;
    map: string;
  }, $$PARAM_1$$: OutputOptions, $$PARAM_2$$: (logMessage: string) => void) => Promise<any>;
}
export interface RunBuildOptions {
  entry: string;
  assets?: boolean;
  dev?: boolean;
  out?: string;
  bundleOut?: string;
  sourceMapOut?: string;
  onBegin?: () => void;
  onComplete?: () => void;
  onProgress?: (transformedFileCount: number, totalFileCount: number) => void;
  minify?: boolean;
  output?: _RunBuildOptions_output;
  platform?: string;
  sourceMap?: boolean;
  sourceMapUrl?: string;
  customResolverOptions?: CustomResolverOptions;
  customTransformOptions?: CustomTransformOptions;
  unstable_transformProfile?: TransformProfile;
}
export interface RunBuildResult {
  code: string;
  map: string;
  assets?: ReadonlyArray<AssetData>;
}
type BuildCommandOptions = {} | null;
type ServeCommandOptions = {} | null;
export { Terminal, JsonReporter, TerminalReporter };
export type { AssetData } from "./Assets";
export type { Reporter, ReportableEvent } from "./lib/reporting";
export type { TerminalReportableEvent } from "./lib/TerminalReporter";
export type { MetroConfig };
export declare function runMetro(config: InputConfigT, options?: RunMetroOptions): Promise<MetroServer>;
export { loadConfig, mergeConfig, resolveConfig };
export declare const createConnectMiddleware: (config: ConfigT, options?: RunMetroOptions) => Promise<MetroMiddleWare>;
export declare const runServer: (config: ConfigT, $$PARAM_1$$: RunServerOptions) => Promise<RunServerResult>;
export declare const runBuild: (config: ConfigT, $$PARAM_1$$: RunBuildOptions) => Promise<RunBuildResult>;
export declare const buildGraph: (config: InputConfigT, $$PARAM_1$$: BuildGraphOptions) => Promise<ReadOnlyGraph>;
export interface AttachMetroCLIOptions {
  build?: BuildCommandOptions;
  serve?: ServeCommandOptions;
  dependencies?: any;
}
export declare const attachMetroCli: (yargs: Yargs, options?: AttachMetroCLIOptions) => Yargs;
/**
 * Backwards-compatibility with CommonJS consumers using interopRequireDefault.
 * Do not add to this list.
 *
 * @deprecated Default import from 'metro' is deprecated, use named exports.
 */