/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 *
 * @format
 * @oncall react_native
 */

import type { HandleFunction, Server } from "connect";
import type { CacheStore, MetroCache } from "../metro-cache";
import type { CacheManagerFactory } from "../metro-file-map";
import type { CustomResolver } from "../metro-resolver";
import type { JsTransformerConfig } from "../metro-transform-worker";
import type { DeltaResult, Module, ReadOnlyGraph, SerializerOptions, TransformResult } from "../metro/DeltaBundler/types";
import type { Reporter } from "../metro/lib/reporting";
import type MetroServer from "../metro/Server";
import type { IntermediateStackFrame } from "../metro/Server/symbolicate";
export interface _ExtraTransformOptions_transform {
  readonly experimentalImportSupport?: boolean;
  readonly inlineRequires?: {
    readonly blockList: {
      readonly [absoluteModulePath: string]: true;
    };
  } | boolean;
  readonly nonInlinedRequires?: ReadonlyArray<string>;
  readonly unstable_memoizeInlineRequires?: boolean;
  readonly unstable_nonMemoizedInlineRequires?: ReadonlyArray<string>;
}
export interface ExtraTransformOptions {
  readonly preloadedModules?: {
    readonly [path: string]: true;
  } | false;
  readonly ramGroups?: ReadonlyArray<string>;
  readonly transform?: _ExtraTransformOptions_transform;
}
export interface GetTransformOptionsOpts {
  dev: boolean;
  /**
   * @deprecated Always true
   */
  hot: true;
  platform?: null | string;
}
export type GetTransformOptions = (entryPoints: ReadonlyArray<string>, options: GetTransformOptionsOpts, getDependenciesOf: (absoluteFilePath: string) => Promise<Array<string>>) => Promise<Partial<ExtraTransformOptions>>;
export type Middleware = HandleFunction;
export interface _PerfAnnotations_string {
  readonly [key: string]: string;
}
export interface _PerfAnnotations_int {
  readonly [key: string]: number;
}
export interface _PerfAnnotations_double {
  readonly [key: string]: number;
}
export interface _PerfAnnotations_bool {
  readonly [key: string]: boolean;
}
export interface _PerfAnnotations_string_array {
  readonly [key: string]: ReadonlyArray<string>;
}
export interface _PerfAnnotations_int_array {
  readonly [key: string]: ReadonlyArray<number>;
}
export interface _PerfAnnotations_double_array {
  readonly [key: string]: ReadonlyArray<number>;
}
export interface _PerfAnnotations_bool_array {
  readonly [key: string]: ReadonlyArray<boolean>;
}
export interface PerfAnnotations {
  string?: _PerfAnnotations_string;
  int?: _PerfAnnotations_int;
  double?: _PerfAnnotations_double;
  bool?: _PerfAnnotations_bool;
  string_array?: _PerfAnnotations_string_array;
  int_array?: _PerfAnnotations_int_array;
  double_array?: _PerfAnnotations_double_array;
  bool_array?: _PerfAnnotations_bool_array;
}
export interface PerfLoggerPointOptions {
  readonly timestamp?: number;
}
export interface PerfLogger {
  point(name: string, opts?: PerfLoggerPointOptions): void;
  annotate(annotations: PerfAnnotations): void;
  subSpan(label: string): PerfLogger;
}
export interface RootPerfLogger extends PerfLogger {
  start(opts?: PerfLoggerPointOptions): void;
  end(status: "SUCCESS" | "FAIL" | "CANCEL", opts?: PerfLoggerPointOptions): void;
}
export interface PerfLoggerFactoryOptions {
  readonly key?: number;
}
export type PerfLoggerFactory = (type: "START_UP" | "BUNDLING_REQUEST" | "HMR", opts?: PerfLoggerFactoryOptions) => RootPerfLogger;
export interface _ResolverConfigT_extraNodeModules {
  [name: string]: string;
}
export interface _ResolverConfigT_unstable_conditionsByPlatform {
  readonly [platform: string]: ReadonlyArray<string>;
}
export interface ResolverConfigT {
  assetExts: ReadonlyArray<string>;
  assetResolutions: ReadonlyArray<string>;
  blacklistRE?: RegExp | Array<RegExp>;
  blockList?: RegExp | Array<RegExp>;
  disableHierarchicalLookup: boolean;
  dependencyExtractor?: null | string;
  emptyModulePath: string;
  enableGlobalPackages: boolean;
  extraNodeModules: _ResolverConfigT_extraNodeModules;
  hasteImplModulePath?: null | string;
  nodeModulesPaths: ReadonlyArray<string>;
  platforms: ReadonlyArray<string>;
  resolveRequest?: null | CustomResolver;
  resolverMainFields: ReadonlyArray<string>;
  sourceExts: ReadonlyArray<string>;
  unstable_conditionNames: ReadonlyArray<string>;
  unstable_conditionsByPlatform: _ResolverConfigT_unstable_conditionsByPlatform;
  unstable_enablePackageExports: boolean;
  useWatchman: boolean;
  requireCycleIgnorePatterns: ReadonlyArray<RegExp>;
}
export interface SerializerConfigT {
  createModuleIdFactory: () => (path: string) => number;
  customSerializer?: null | ((entryPoint: string, preModules: ReadonlyArray<Module>, graph: ReadOnlyGraph, options: SerializerOptions) => Promise<string | {
    code: string;
    map: string;
  }>);
  experimentalSerializerHook: (graph: ReadOnlyGraph, delta: DeltaResult) => any;
  getModulesRunBeforeMainModule: (entryFilePath: string) => Array<string>;
  getPolyfills: ($$PARAM_0$$: {
    platform?: null | string;
  }) => ReadonlyArray<string>;
  getRunModuleStatement: (moduleId: number | string, globalPrefix: string) => string;
  polyfillModuleNames: ReadonlyArray<string>;
  processModuleFilter: (modules: Module) => boolean;
  isThirdPartyModule: (module: {
    readonly path: string;
  }) => boolean;
}
export interface _TransformerConfigT_transformVariants {
  readonly [name: string]: Partial<ExtraTransformOptions>;
}
export interface TransformerConfigT extends JsTransformerConfig {
  getTransformOptions: GetTransformOptions;
  transformVariants: _TransformerConfigT_transformVariants;
  publicPath: string;
  unstable_workerThreads: boolean;
}
export interface MetalConfigT {
  cacheVersion: string;
  fileMapCacheDirectory?: string;
  hasteMapCacheDirectory?: string;
  unstable_fileMapCacheManagerFactory?: CacheManagerFactory;
  maxWorkers: number;
  unstable_perfLoggerFactory?: null | undefined | PerfLoggerFactory;
  projectRoot: string;
  stickyWorkers: boolean;
  transformerPath: string;
  reporter: Reporter;
  resetCache: boolean;
  watchFolders: ReadonlyArray<string>;
}
type CacheStoresConfigT = ReadonlyArray<CacheStore<TransformResult>>;
export interface ServerConfigT {
  /** @deprecated */
  enhanceMiddleware: ($$PARAM_0$$: Middleware, $$PARAM_1$$: MetroServer) => Middleware | Server;
  forwardClientLogs: boolean;
  port: number;
  rewriteRequestUrl: ($$PARAM_0$$: string) => string;
  unstable_serverRoot?: null | string;
  useGlobalHotkey: boolean;
  verifyConnections: boolean;
}
export interface SymbolicatorConfigT {
  customizeFrame: ($$PARAM_0$$: {
    readonly file?: null | string;
    readonly lineNumber?: null | number;
    readonly column?: null | number;
    readonly methodName?: null | string;
  }) => (null | undefined | {
    readonly collapse?: boolean;
  }) | Promise<null | undefined | {
    readonly collapse?: boolean;
  }>;
  customizeStack: ($$PARAM_0$$: Array<IntermediateStackFrame>, $$PARAM_1$$: any) => Array<IntermediateStackFrame> | Promise<Array<IntermediateStackFrame>>;
}
export interface _WatcherConfigT_healthCheck {
  readonly enabled: boolean;
  readonly interval: number;
  readonly timeout: number;
  readonly filePrefix: string;
}
export interface _WatcherConfigT_unstable_autoSaveCache {
  readonly enabled: boolean;
  readonly debounceMs?: number;
}
export interface _WatcherConfigT_watchman {
  readonly deferStates: ReadonlyArray<string>;
}
export interface WatcherConfigT {
  additionalExts: ReadonlyArray<string>;
  healthCheck: _WatcherConfigT_healthCheck;
  unstable_autoSaveCache: _WatcherConfigT_unstable_autoSaveCache;
  unstable_lazySha1: boolean;
  unstable_workerThreads: boolean;
  watchman: _WatcherConfigT_watchman;
}
export interface _InputConfigT_watcher extends Readonly<Partial<Omit<WatcherConfigT, "healthCheck" | "unstable_autoSaveCache" | "watchman">>> {
  readonly healthCheck?: Partial<Readonly<WatcherConfigT["healthCheck"]>>;
  readonly unstable_autoSaveCache?: Partial<Readonly<WatcherConfigT["unstable_autoSaveCache"]>>;
  readonly watchman?: Partial<Readonly<WatcherConfigT["watchman"]>>;
}
export interface InputConfigT extends Readonly<Partial<MetalConfigT>> {
  readonly cacheStores?: CacheStoresConfigT | (($$PARAM_0$$: MetroCache) => CacheStoresConfigT);
  readonly resolver?: Readonly<Partial<ResolverConfigT>>;
  readonly server?: Readonly<Partial<ServerConfigT>>;
  readonly serializer?: Readonly<Partial<SerializerConfigT>>;
  readonly symbolicator?: Readonly<Partial<SymbolicatorConfigT>>;
  readonly transformer?: Readonly<Partial<TransformerConfigT>>;
  readonly watcher?: _InputConfigT_watcher;
}
export type MetroConfig = InputConfigT;
export interface ConfigT extends Readonly<MetalConfigT> {
  readonly cacheStores: CacheStoresConfigT;
  readonly resolver: Readonly<ResolverConfigT>;
  readonly server: Readonly<ServerConfigT>;
  readonly serializer: Readonly<SerializerConfigT>;
  readonly symbolicator: Readonly<SymbolicatorConfigT>;
  readonly transformer: Readonly<TransformerConfigT>;
  readonly watcher: Readonly<WatcherConfigT>;
}
export interface YargArguments {
  readonly config?: string;
  readonly cwd?: string;
  readonly port?: string | number;
  readonly host?: string;
  readonly projectRoot?: string;
  readonly watchFolders?: Array<string>;
  readonly assetExts?: Array<string>;
  readonly sourceExts?: Array<string>;
  readonly platforms?: Array<string>;
  readonly "max-workers"?: string | number;
  readonly maxWorkers?: string | number;
  readonly transformer?: string;
  readonly "reset-cache"?: boolean;
  readonly resetCache?: boolean;
  readonly verbose?: boolean;
}