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

import type { Options as DeltaBundlerOptions, TransformInputOptions } from "../DeltaBundler/types";
import type { TransformProfile } from "../../metro-babel-transformer";
import type { CustomResolverOptions } from "../../metro-resolver";
import type { MetroSourceMapSegmentTuple, MixedSourceMap } from "../../metro-source-map";
import type { CustomTransformOptions, MinifierOptions } from "../../metro-transform-worker";
type MetroSourceMapOrMappings = MixedSourceMap | Array<MetroSourceMapSegmentTuple>;
export declare enum SourcePathsMode {
  Absolute = "absolute",
  ServerUrl = "url-server",
}
export declare namespace SourcePathsMode {
  export function cast(value: string | null | undefined): SourcePathsMode;
  export function isValid(value: string | null | undefined): value is SourcePathsMode;
  export function members(): IterableIterator<SourcePathsMode>;
  export function getName(value: SourcePathsMode): string;
}
export interface BundleOptions {
  readonly customResolverOptions: CustomResolverOptions;
  customTransformOptions: CustomTransformOptions;
  dev: boolean;
  entryFile: string;
  readonly excludeSource: boolean;
  readonly inlineSourceMap: boolean;
  readonly lazy: boolean;
  minify: boolean;
  readonly modulesOnly: boolean;
  onProgress?: null | ((doneCont: number, totalCount: number) => any);
  readonly platform?: null | string;
  readonly runModule: boolean;
  readonly shallow: boolean;
  sourceMapUrl?: null | string;
  sourceUrl?: null | string;
  createModuleIdFactory?: () => (path: string) => number;
  readonly unstable_transformProfile: TransformProfile;
  readonly sourcePaths: SourcePathsMode;
}
export interface BuildOptions {
  readonly withAssets?: boolean;
}
export interface ResolverInputOptions {
  readonly customResolverOptions?: CustomResolverOptions;
  readonly dev: boolean;
}
export interface SerializerOptions {
  readonly sourceMapUrl?: null | string;
  readonly sourceUrl?: null | string;
  readonly runModule: boolean;
  readonly excludeSource: boolean;
  readonly inlineSourceMap: boolean;
  readonly modulesOnly: boolean;
  readonly sourcePaths: SourcePathsMode;
}
export interface GraphOptions {
  readonly lazy: boolean;
  readonly shallow: boolean;
}
export interface SplitBundleOptions {
  readonly entryFile: string;
  readonly resolverOptions: ResolverInputOptions;
  readonly transformOptions: TransformInputOptions;
  readonly serializerOptions: SerializerOptions;
  readonly graphOptions: GraphOptions;
  readonly onProgress: DeltaBundlerOptions["onProgress"];
}
export interface ModuleGroups {
  groups: Map<number, Set<number>>;
  modulesById: Map<number, ModuleTransportLike>;
  modulesInGroups: Set<number>;
}
export interface ModuleTransportLike {
  readonly code: string;
  readonly id: number;
  readonly map?: null | MetroSourceMapOrMappings;
  readonly name?: string;
  readonly sourcePath: string;
}
export interface ModuleTransportLikeStrict {
  readonly code: string;
  readonly id: number;
  readonly map?: null | MetroSourceMapOrMappings;
  readonly name?: string;
  readonly sourcePath: string;
}
export interface RamModuleTransport extends ModuleTransportLikeStrict {
  readonly source: string;
  readonly type: string;
}
export interface OutputOptions {
  bundleOutput: string;
  bundleEncoding?: "utf8" | "utf16le" | "ascii";
  dev?: boolean;
  indexedRamBundle?: boolean;
  platform: string;
  sourcemapOutput?: string;
  sourcemapSourcesRoot?: string;
  sourcemapUseAbsolutePath?: boolean;
}
export interface RequestOptions {
  readonly entryFile?: string;
  readonly inlineSourceMap?: boolean;
  readonly sourceMapUrl?: string;
  readonly dev?: boolean;
  readonly minify?: boolean;
  readonly platform?: string;
  readonly createModuleIdFactory?: () => (path: string) => number;
  readonly onProgress?: (transformedFileCount: number, totalFileCount: number) => void;
  readonly customResolverOptions?: CustomResolverOptions;
  readonly customTransformOptions?: CustomTransformOptions;
  readonly unstable_transformProfile?: TransformProfile;
}
export type { MinifierOptions };