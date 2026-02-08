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

import type { BundlerResolution, TransformResultDependency } from "../../DeltaBundler/types";
import type { Reporter } from "../../lib/reporting";
import type { ResolverInputOptions } from "../../shared/types";
import type { CustomResolver, DoesFileExist, FileCandidates, FileSystemLookup, Resolution, ResolveAsset } from "../../../metro-resolver";
import type { PackageJson } from "../../../metro-resolver/types";
export type DirExistsFn = (filePath: string) => boolean;
export type Packageish = {
  path: string;
  read(): PackageJson;
};
export type Moduleish = {
  readonly path: string;
};
export type PackageishCache<TPackage> = {
  getPackage(name: string, platform?: string, supportsNativePlatform?: boolean): TPackage;
  getPackageOf(absolutePath: string): null | undefined | {
    pkg: TPackage;
    packageRelativePath: string;
  };
};
export interface _Options_unstable_conditionsByPlatform<TPackage> {
  readonly [platform: string]: ReadonlyArray<string>;
}
export interface Options<TPackage> {
  readonly assetExts: ReadonlySet<string>;
  readonly dirExists: DirExistsFn;
  readonly disableHierarchicalLookup: boolean;
  readonly doesFileExist: DoesFileExist;
  readonly emptyModulePath: string;
  readonly extraNodeModules?: null | Object;
  readonly fileSystemLookup: FileSystemLookup;
  readonly getHasteModulePath: (name: string, platform: null | undefined | string) => null | undefined | string;
  readonly getHastePackagePath: (name: string, platform: null | undefined | string) => null | undefined | string;
  readonly mainFields: ReadonlyArray<string>;
  readonly packageCache: PackageishCache<TPackage>;
  readonly nodeModulesPaths: ReadonlyArray<string>;
  readonly preferNativePlatform: boolean;
  readonly projectRoot: string;
  readonly reporter: Reporter;
  readonly resolveAsset: ResolveAsset;
  readonly resolveRequest?: null | CustomResolver;
  readonly sourceExts: ReadonlyArray<string>;
  readonly unstable_conditionNames: ReadonlyArray<string>;
  readonly unstable_conditionsByPlatform: _Options_unstable_conditionsByPlatform<TPackage>;
  readonly unstable_enablePackageExports: boolean;
}
export declare class ModuleResolver<TPackage extends Packageish> {
  _options: Options<TPackage>;
  _projectRootFakeModulePath: string;
  _cachedEmptyModule: null | undefined | BundlerResolution;
  constructor(options: Options<TPackage>);
  _getEmptyModule(): BundlerResolution;
  resolveDependency(originModulePath: string, dependency: TransformResultDependency, allowHaste: boolean, platform: string | null, resolverOptions: ResolverInputOptions): BundlerResolution;
  _getPackage: any;
  _getPackageForModule: any;
  _getFileResolvedModule(resolution: Resolution): BundlerResolution;
  _logWarning: any;
  _removeRoot(candidates: FileCandidates): FileCandidates;
}
export declare class UnableToResolveError extends Error {
  originModulePath: string;
  targetModuleName: string;
  cause: null | undefined | Error;
  readonly type: "UnableToResolveError";
  constructor(originModulePath: string, targetModuleName: string, message: string, options?: {
    readonly dependency?: null | undefined | TransformResultDependency;
    readonly cause?: Error;
  });
  buildCodeFrameMessage(dependency: null | undefined | TransformResultDependency): null | undefined | string;
}