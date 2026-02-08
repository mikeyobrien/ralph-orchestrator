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

import type { PerfLogger, PerfLoggerFactory, RootPerfLogger } from "../metro-config";
export type { PerfLoggerFactory, PerfLogger };
export interface BuildParameters {
  readonly computeDependencies: boolean;
  readonly computeSha1: boolean;
  readonly enableHastePackages: boolean;
  readonly enableSymlinks: boolean;
  readonly extensions: ReadonlyArray<string>;
  readonly forceNodeFilesystemAPI: boolean;
  readonly ignorePattern: RegExp;
  readonly plugins: ReadonlyArray<FileMapPlugin>;
  readonly retainAllFiles: boolean;
  readonly rootDir: string;
  readonly roots: ReadonlyArray<string>;
  readonly skipPackageJson: boolean;
  readonly dependencyExtractor?: null | string;
  readonly hasteImplModulePath?: null | string;
  readonly cacheBreaker: string;
}
export interface BuildResult {
  fileSystem: FileSystem;
  hasteMap: HasteMap;
  mockMap?: null | MockMap;
}
export interface CacheData {
  readonly clocks: WatchmanClocks;
  readonly fileSystemData: any;
  readonly plugins: ReadonlyMap<string, V8Serializable>;
}
export interface CacheManager {
  /**
   * Called during startup to load initial state, if available. Provided to
   * a crawler, which will return the delta between the initial state and the
   * current file system state.
   */
  read(): Promise<null | undefined | CacheData>;
  /**
   * Called when metro-file-map `build()` has applied changes returned by the
   * crawler - i.e. internal state reflects the current file system state.
   *
   * getSnapshot may be retained and called at any time before end(), such as
   * in response to eventSource 'change' events.
   */
  write(getSnapshot: () => CacheData, opts: CacheManagerWriteOptions): Promise<void>;
  /**
   * The last call that will be made to this CacheManager. Any handles should
   * be closed by the time this settles.
   */
  end(): Promise<void>;
}
export interface CacheManagerEventSource {
  onChange(listener: () => void): () => void;
}
export type CacheManagerFactory = (options: CacheManagerFactoryOptions) => CacheManager;
export interface CacheManagerFactoryOptions {
  readonly buildParameters: BuildParameters;
}
export interface CacheManagerWriteOptions {
  readonly changedSinceCacheRead: boolean;
  readonly eventSource: CacheManagerEventSource;
  readonly onWriteError: ($$PARAM_0$$: Error) => void;
}
export type CanonicalPath = string;
export interface ChangeEvent {
  logger?: null | RootPerfLogger;
  eventsQueue: EventsQueue;
}
export interface ChangeEventMetadata {
  modifiedTime?: null | number;
  size?: null | number;
  type?: "f" | "d" | "l";
}
export type Console = typeof global.console;
export interface _CrawlerOptions_previousState {
  readonly clocks: ReadonlyMap<CanonicalPath, WatchmanClockSpec>;
  readonly fileSystem: FileSystem;
}
export interface CrawlerOptions {
  abortSignal?: null | AbortSignal;
  computeSha1: boolean;
  console: Console;
  extensions: ReadonlyArray<string>;
  forceNodeFilesystemAPI: boolean;
  ignore: IgnoreMatcher;
  includeSymlinks: boolean;
  perfLogger?: null | undefined | PerfLogger;
  previousState: _CrawlerOptions_previousState;
  rootDir: string;
  roots: ReadonlyArray<string>;
  onStatus: (status: WatcherStatus) => void;
}
export interface DependencyExtractor {
  extract: (content: string, absoluteFilePath: string, defaultExtractor?: DependencyExtractor["extract"]) => Set<string>;
  getCacheKey: () => string;
}
export type WatcherStatus = {
  type: "watchman_slow_command";
  timeElapsed: number;
  command?: "watch-project" | "query";
} | {
  type: "watchman_slow_command_complete";
  timeElapsed: number;
  command?: "watch-project" | "query";
} | {
  type: "watchman_warning";
  warning: any;
  command?: "watch-project" | "query";
};
export type DuplicatesSet = Map<string, number>;
export type DuplicatesIndex = Map<string, Map<string, DuplicatesSet>>;
export type EventsQueue = Array<{
  filePath: Path;
  metadata: ChangeEventMetadata;
  type: string;
}>;
export interface FileMapDelta {
  readonly removed: Iterable<[CanonicalPath, FileMetadata]>;
  readonly addedOrModified: Iterable<[CanonicalPath, FileMetadata]>;
}
export interface FileSystemState {
  metadataIterator(opts: {
    readonly includeNodeModules: boolean;
    readonly includeSymlinks: boolean;
  }): Iterable<{
    baseName: string;
    canonicalPath: string;
    metadata: FileMetadata;
  }>;
}
export interface FileMapPluginInitOptions<SerializableState> {
  readonly files: FileSystemState;
  readonly pluginState?: null | SerializableState;
}
type V8Serializable = {};
export interface FileMapPlugin<SerializableState = V8Serializable> {
  readonly name: string;
  initialize(initOptions: FileMapPluginInitOptions<SerializableState>): Promise<void>;
  assertValid(): void;
  bulkUpdate(delta: FileMapDelta): Promise<void>;
  getSerializableSnapshot(): SerializableState;
  onRemovedFile(relativeFilePath: string, fileMetadata: FileMetadata): void;
  onNewOrModifiedFile(relativeFilePath: string, fileMetadata: FileMetadata): void;
  getCacheKey(): string;
}
export interface HType {
  MTIME: 0;
  SIZE: 1;
  VISITED: 2;
  DEPENDENCIES: 3;
  SHA1: 4;
  SYMLINK: 5;
  ID: 6;
  PATH: 0;
  TYPE: 1;
  MODULE: 0;
  PACKAGE: 1;
  GENERIC_PLATFORM: "g";
  NATIVE_PLATFORM: "native";
  DEPENDENCY_DELIM: "\0";
}
export type HTypeValue = HType[keyof HType];
export type IgnoreMatcher = (item: string) => boolean;
export type FileData = Map<CanonicalPath, FileMetadata>;
export type FileMetadata = [null | undefined | number, number, 0 | 1, string, null | undefined | string, 0 | 1 | string, string];
export interface FileStats {
  readonly fileType?: "f" | "l";
  readonly modifiedTime?: null | number;
  readonly size?: null | number;
}
export interface FileSystem {
  exists(file: Path): boolean;
  getAllFiles(): Array<Path>;
  getDependencies(file: Path): null | undefined | Array<string>;
  getDifference(files: FileData): {
    changedFiles: FileData;
    removedFiles: Set<string>;
  };
  getModuleName(file: Path): null | undefined | string;
  getSerializableSnapshot(): CacheData["fileSystemData"];
  getSha1(file: Path): null | undefined | string;
  getOrComputeSha1(file: Path): Promise<null | undefined | {
    sha1: string;
    content?: Buffer;
  }>;
  /**
   * Given a start path (which need not exist), a subpath and type, and
   * optionally a 'breakOnSegment', performs the following:
   *
   * X = mixedStartPath
   * do
   *   if basename(X) === opts.breakOnSegment
   *     return null
   *   if X + subpath exists and has type opts.subpathType
   *     return {
   *       absolutePath: realpath(X + subpath)
   *       containerRelativePath: relative(mixedStartPath, X)
   *     }
   *   X = dirname(X)
   * while X !== dirname(X)
   *
   * If opts.invalidatedBy is given, collects all absolute, real paths that if
   * added or removed may invalidate this result.
   *
   * Useful for finding the closest package scope (subpath: package.json,
   * type f, breakOnSegment: node_modules) or closest potential package root
   * (subpath: node_modules/pkg, type: d) in Node.js resolution.
   */
  hierarchicalLookup(mixedStartPath: string, subpath: string, opts: {
    breakOnSegment?: null | string;
    invalidatedBy?: null | Set<string>;
    subpathType?: "f" | "d";
  }): null | undefined | {
    absolutePath: string;
    containerRelativePath: string;
  };
  /**
   * Analogous to posix lstat. If the file at `file` is a symlink, return
   * information about the symlink without following it.
   */
  linkStats(file: Path): null | undefined | FileStats;
  /**
   * Return information about the given path, whether a directory or file.
   * Always follow symlinks, and return a real path if it exists.
   */
  lookup(mixedPath: Path): LookupResult;
  matchFiles(opts: {
    filter?: RegExp | null;
    filterCompareAbsolute?: boolean;
    filterComparePosix?: boolean;
    follow?: boolean;
    recursive?: boolean;
    rootDir?: Path | null;
  }): Iterable<Path>;
}
export type Glob = string;
export type LookupResult = {
  exists: false;
  links: ReadonlySet<string>;
  missing: string;
} | {
  exists: true;
  links: ReadonlySet<string>;
  realPath: string;
  type?: "d" | "f";
};
export interface MockMap {
  getMockModule(name: string): null | undefined | Path;
}
export interface HasteConflict {
  id: string;
  platform?: string | null;
  absolutePaths: Array<string>;
  type?: "duplicate" | "shadowing";
}
export interface HasteMap {
  getModule(name: string, platform?: null | undefined | string, supportsNativePlatform?: null | undefined | boolean, type?: null | undefined | HTypeValue): null | undefined | Path;
  getPackage(name: string, platform: null | undefined | string, _supportsNativePlatform: null | undefined | boolean): null | undefined | Path;
  computeConflicts(): Array<HasteConflict>;
}
export type HasteMapData = Map<string, HasteMapItem>;
export interface HasteMapItem {
  [platform: string]: HasteMapItemMetadata;
}
export type HasteMapItemMetadata = [string, number];
export interface MutableFileSystem extends FileSystem {
  remove(filePath: Path): null | undefined | FileMetadata;
  addOrModify(filePath: Path, fileMetadata: FileMetadata): void;
  bulkAddOrModify(addedOrModifiedFiles: FileData): void;
}
export type Path = string;
export type ProcessFileFunction = (absolutePath: string, metadata: FileMetadata, request: {
  readonly computeSha1: boolean;
}) => null | undefined | Buffer;
export interface RawMockMap {
  readonly duplicates: Map<string, Set<string>>;
  readonly mocks: Map<string, Path>;
  readonly version: number;
}
export interface ReadOnlyRawMockMap {
  readonly duplicates: ReadonlyMap<string, ReadonlySet<string>>;
  readonly mocks: ReadonlyMap<string, Path>;
  readonly version: number;
}
export interface WatcherBackend {
  getPauseReason(): null | undefined | string;
  onError($$PARAM_0$$: (error: Error) => void): () => void;
  onFileEvent($$PARAM_0$$: (event: WatcherBackendChangeEvent) => void): () => void;
  startWatching(): Promise<void>;
  stopWatching(): Promise<void>;
}
export type ChangeEventClock = [string, string];
export type WatcherBackendChangeEvent = {
  readonly event: "touch";
  readonly clock?: ChangeEventClock;
  readonly relativePath: string;
  readonly root: string;
  readonly metadata: ChangeEventMetadata;
} | {
  readonly event: "delete";
  readonly clock?: ChangeEventClock;
  readonly relativePath: string;
  readonly root: string;
  readonly metadata?: void;
};
export interface WatcherBackendOptions {
  readonly ignored?: null | RegExp;
  readonly globs: ReadonlyArray<string>;
  readonly dot: boolean;
}
export type WatchmanClockSpec = string | {
  readonly scm: {
    readonly "mergebase-with": string;
  };
};
export type WatchmanClocks = Map<Path, WatchmanClockSpec>;
export interface WorkerMessage {
  readonly computeDependencies: boolean;
  readonly computeSha1: boolean;
  readonly dependencyExtractor?: null | undefined | string;
  readonly enableHastePackages: boolean;
  readonly filePath: string;
  readonly hasteImplModulePath?: null | undefined | string;
  readonly maybeReturnContent: boolean;
}
export interface WorkerMetadata {
  readonly dependencies?: null | undefined | ReadonlyArray<string>;
  readonly id?: null | undefined | string;
  readonly sha1?: null | undefined | string;
  readonly content?: null | undefined | Buffer;
}
export interface WorkerSetupArgs {}