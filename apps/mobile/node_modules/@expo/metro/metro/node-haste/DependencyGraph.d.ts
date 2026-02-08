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

import type { BundlerResolution, TransformResultDependency } from "../DeltaBundler/types";
import type { ResolverInputOptions } from "../shared/types";
import type Package from "./Package";
import type { ConfigT } from "../../metro-config";
import { ModuleResolver } from "./DependencyGraph/ModuleResolution";
import { PackageCache } from "./PackageCache";
import EventEmitter from "node:events";
import type MetroFileMap from "../../metro-file-map";
import type { ChangeEvent, FileSystem, HasteMap, HealthCheckResult, WatcherStatus } from "../../metro-file-map";
declare class DependencyGraph extends EventEmitter {
  _config: ConfigT;
  _haste: MetroFileMap;
  _fileSystem: FileSystem;
  _hasteMap: HasteMap;
  _moduleResolver: ModuleResolver<Package>;
  _resolutionCache: Map<string | symbol, Map<string | symbol, Map<string | symbol, Map<string | symbol, BundlerResolution>>>>;
  _initializedPromise: Promise<void>;
  constructor(config: ConfigT, options?: {
    readonly hasReducedPerformance?: boolean;
    readonly watch?: boolean;
  });
  _onWatcherHealthCheck(result: HealthCheckResult): void;
  _onWatcherStatus(status: WatcherStatus): void;
  ready(): Promise<void>;
  static load(config: ConfigT, options?: {
    readonly hasReducedPerformance?: boolean;
    readonly watch?: boolean;
  }): Promise<DependencyGraph>;
  _onHasteChange($$PARAM_0$$: ChangeEvent): void;
  _createModuleResolver(): void;
  _getClosestPackage(absoluteModulePath: string): null | undefined | {
    packageJsonPath: string;
    packageRelativePath: string;
  };
  _createPackageCache(): PackageCache;
  getAllFiles(): Array<string>;
  getOrComputeSha1(mixedPath: string): Promise<{
    content?: Buffer;
    sha1: string;
  }>;
  getWatcher(): EventEmitter;
  end(): Promise<void>;
  matchFilesWithContext(from: string, context: {
    readonly recursive: boolean;
    readonly filter: RegExp;
  }): Iterable<string>;
  resolveDependency(originModulePath: string, dependency: TransformResultDependency, platform: string | null, resolverOptions: ResolverInputOptions, $$PARAM_4$$: {
    assumeFlatNodeModules: boolean;
  }): BundlerResolution;
  _doesFileExist: any;
  getHasteName(filePath: string): string;
  getDependencies(filePath: string): Array<string>;
}
export default DependencyGraph;