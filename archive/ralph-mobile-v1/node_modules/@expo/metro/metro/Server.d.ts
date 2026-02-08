import type { AssetData } from "./Assets";
import type { ExplodedSourceMap } from "./DeltaBundler/Serializers/getExplodedSourceMap";
import type { RamBundleInfo } from "./DeltaBundler/Serializers/getRamBundleInfo";
import type { Module, ReadOnlyDependencies, ReadOnlyGraph, TransformInputOptions } from "./DeltaBundler/types";
import type { RevisionId } from "./IncrementalBundler";
import type { GraphId } from "./lib/getGraphId";
import type { Reporter } from "./lib/reporting";
import type { BuildOptions, BundleOptions, GraphOptions, ResolverInputOptions, SplitBundleOptions } from "./shared/types";
import type { IncomingMessage } from "connect";
import type { ServerResponse } from "node:http";
import type { ConfigT, RootPerfLogger } from "../metro-config";
import type { ActionLogEntryData, ActionStartLogEntry } from "../metro-core/Logger";
import type { CustomResolverOptions } from "../metro-resolver/types";
import type { CustomTransformOptions } from "../metro-transform-worker";
import IncrementalBundler from "./IncrementalBundler";
import MultipartResponse from "./Server/MultipartResponse";
import { SourcePathsMode } from "./shared/types";
import { Logger } from "../metro-core";
export interface SegmentLoadData {
  [$$Key$$: number]: [Array<number>, null | undefined | number];
}
export interface BundleMetadata {
  hash: string;
  otaBuildNumber?: null | string;
  mobileConfigs: Array<string>;
  segmentHashes: Array<string>;
  segmentLoadData: SegmentLoadData;
}
export interface ProcessStartContext extends SplitBundleOptions {
  readonly buildNumber: number;
  readonly bundleOptions: BundleOptions;
  readonly graphId: GraphId;
  readonly graphOptions: GraphOptions;
  readonly mres?: MultipartResponse | ServerResponse;
  readonly req: IncomingMessage;
  readonly revisionId?: null | undefined | RevisionId;
  readonly bundlePerfLogger: RootPerfLogger;
  readonly requestStartTimestamp: number;
}
export interface ProcessDeleteContext {
  readonly graphId: GraphId;
  readonly req: IncomingMessage;
  readonly res: ServerResponse;
}
export interface ProcessEndContext<T> extends ProcessStartContext {
  readonly result: T;
}
export interface ServerOptions {
  readonly hasReducedPerformance?: boolean;
  readonly onBundleBuilt?: (bundlePath: string) => void;
  readonly watch?: boolean;
}
declare class Server {
  _bundler: IncrementalBundler;
  _config: ConfigT;
  _createModuleId: (path: string) => number;
  _isEnded: boolean;
  _logger: typeof Logger;
  _nextBundleBuildNumber: number;
  _platforms: Set<string>;
  _reporter: Reporter;
  _serverOptions: ServerOptions | void;
  _allowedSuffixesForSourceRequests: ReadonlyArray<string>;
  _sourceRequestRoutingMap: ReadonlyArray<[any, any]>;
  constructor(config: ConfigT, options?: ServerOptions);
  end(): Promise<void>;
  getBundler(): IncrementalBundler;
  getCreateModuleId(): (path: string) => number;
  _serializeGraph($$PARAM_0$$: {
    readonly splitOptions: SplitBundleOptions;
    readonly prepend: ReadonlyArray<Module>;
    readonly graph: ReadOnlyGraph;
  }): Promise<{
    code: string;
    map: string;
  }>;
  build(bundleOptions: BundleOptions, $$PARAM_1$$: BuildOptions): Promise<{
    code: string;
    map: string;
    assets?: ReadonlyArray<AssetData>;
  }>;
  getRamBundleInfo(options: BundleOptions): Promise<RamBundleInfo>;
  getAssets(options: BundleOptions): Promise<ReadonlyArray<AssetData>>;
  _getAssetsFromDependencies(dependencies: ReadOnlyDependencies, platform: null | undefined | string): Promise<ReadonlyArray<AssetData>>;
  getOrderedDependencyPaths(options: {
    readonly dev: boolean;
    readonly entryFile: string;
    readonly minify: boolean;
    readonly platform?: null | string;
  }): Promise<Array<string>>;
  _rangeRequestMiddleware(req: IncomingMessage, res: ServerResponse, data: string | Buffer, assetPath: string): Buffer | string;
  _processSingleAssetRequest(req: IncomingMessage, res: ServerResponse): Promise<void>;
  processRequest: ($$PARAM_0$$: IncomingMessage, $$PARAM_1$$: ServerResponse, $$PARAM_2$$: (e: null | undefined | Error) => void) => void;
  _parseOptions(url: string): BundleOptions;
  _rewriteAndNormalizeUrl(requestUrl: string): string;
  _processRequest(req: IncomingMessage, res: ServerResponse, next: ($$PARAM_0$$: null | undefined | Error) => void): Promise<void>;
  _processSourceRequest(relativeFilePathname: string, rootDir: string, res: ServerResponse): Promise<void>;
  _createRequestProcessor<T>($$PARAM_0$$: {
    readonly bundleType?: "assets" | "bundle" | "map";
    readonly createStartEntry: (context: ProcessStartContext) => ActionLogEntryData;
    readonly createEndEntry: (context: ProcessEndContext<T>) => Partial<ActionStartLogEntry>;
    readonly build: (context: ProcessStartContext) => Promise<T>;
    readonly delete?: (context: ProcessDeleteContext) => Promise<void>;
    readonly finish: (context: ProcessEndContext<T>) => void;
  }): (req: IncomingMessage, res: ServerResponse, bundleOptions: BundleOptions, buildContext: {
    readonly buildNumber: number;
    readonly bundlePerfLogger: RootPerfLogger;
  }) => Promise<void>;
  _processBundleRequest: (req: IncomingMessage, res: ServerResponse, bundleOptions: BundleOptions, buildContext: {
    readonly buildNumber: number;
    readonly bundlePerfLogger: RootPerfLogger;
  }) => Promise<void>;
  _getSortedModules(graph: ReadOnlyGraph): ReadonlyArray<Module>;
  _processSourceMapRequest: (req: IncomingMessage, res: ServerResponse, bundleOptions: BundleOptions, buildContext: {
    readonly buildNumber: number;
    readonly bundlePerfLogger: RootPerfLogger;
  }) => Promise<void>;
  _processAssetsRequest: (req: IncomingMessage, res: ServerResponse, bundleOptions: BundleOptions, buildContext: {
    readonly buildNumber: number;
    readonly bundlePerfLogger: RootPerfLogger;
  }) => Promise<void>;
  _symbolicate(req: IncomingMessage, res: ServerResponse): Promise<void>;
  _explodedSourceMapForBundleOptions(bundleOptions: BundleOptions): Promise<ExplodedSourceMap>;
  _resolveRelativePath(filePath: string, $$PARAM_1$$: {
    readonly relativeTo?: "project" | "server";
    readonly resolverOptions: ResolverInputOptions;
    readonly transformOptions: TransformInputOptions;
  }): Promise<string>;
  getNewBuildNumber(): number;
  getPlatforms(): ReadonlyArray<string>;
  getWatchFolders(): ReadonlyArray<string>;
  static DEFAULT_GRAPH_OPTIONS: {
    readonly customResolverOptions: CustomResolverOptions;
    readonly customTransformOptions: CustomTransformOptions;
    readonly dev: boolean;
    readonly minify: boolean;
    readonly unstable_transformProfile: "default";
  };
  static DEFAULT_BUNDLE_OPTIONS: {
    excludeSource: false;
    inlineSourceMap: false;
    lazy: false;
    modulesOnly: false;
    onProgress: null;
    runModule: true;
    shallow: false;
    sourceMapUrl: null;
    sourceUrl: null;
    sourcePaths: SourcePathsMode;
  } & typeof Server.DEFAULT_GRAPH_OPTIONS;
  _getServerRootDir(): string;
  _getEntryPointAbsolutePath(entryFile: string): string;
  ready(): Promise<void>;
  _shouldAddModuleToIgnoreList(module: Module): boolean;
  _getModuleSourceUrl(module: Module, mode: SourcePathsMode): string;
}
export default Server;