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

import type { IConsumer } from "./Consumer/types";
import { BundleBuilder, createIndexMap } from "./BundleBuilder";
import composeSourceMaps from "./composeSourceMaps";
import Consumer from "./Consumer";
import normalizeSourcePath from "./Consumer/normalizeSourcePath";
import { functionMapBabelPlugin, generateFunctionMap } from "./generateFunctionMap";
import Generator from "./Generator";
export type { IConsumer };
type GeneratedCodeMapping = [number, number];
type SourceMapping = [number, number, number, number];
type SourceMappingWithName = [number, number, number, number, string];
export type MetroSourceMapSegmentTuple = SourceMappingWithName | SourceMapping | GeneratedCodeMapping;
export interface HermesFunctionOffsets {
  [$$Key$$: number]: ReadonlyArray<number>;
}
export type FBSourcesArray = ReadonlyArray<null | undefined | FBSourceMetadata>;
export type FBSourceMetadata = [null | undefined | FBSourceFunctionMap];
export interface FBSourceFunctionMap {
  readonly names: ReadonlyArray<string>;
  readonly mappings: string;
}
export interface _BabelSourceMapSegment_generated {
  readonly column: number;
  readonly line: number;
}
export interface _BabelSourceMapSegment_original {
  readonly column: number;
  readonly line: number;
}
export interface BabelSourceMapSegment {
  readonly generated: _BabelSourceMapSegment_generated;
  readonly original?: _BabelSourceMapSegment_original;
  readonly source?: null | undefined | string;
  readonly name?: null | undefined | string;
}
export interface FBSegmentMap {
  [id: string]: MixedSourceMap;
}
export interface BasicSourceMap {
  readonly file?: string;
  readonly mappings: string;
  readonly names: Array<string>;
  readonly sourceRoot?: string;
  readonly sources: Array<string>;
  readonly sourcesContent?: Array<null | undefined | string>;
  readonly version: number;
  readonly x_facebook_offsets?: Array<number>;
  readonly x_metro_module_paths?: Array<string>;
  readonly x_facebook_sources?: FBSourcesArray;
  readonly x_facebook_segments?: FBSegmentMap;
  readonly x_hermes_function_offsets?: HermesFunctionOffsets;
  readonly x_google_ignoreList?: Array<number>;
}
export interface _IndexMapSection_offset {
  line: number;
  column: number;
}
export interface IndexMapSection {
  map?: IndexMap | BasicSourceMap;
  offset: _IndexMapSection_offset;
}
export interface IndexMap {
  readonly file?: string;
  readonly mappings?: void;
  readonly sourcesContent?: void;
  readonly sections: Array<IndexMapSection>;
  readonly version: number;
  readonly x_facebook_offsets?: Array<number>;
  readonly x_metro_module_paths?: Array<string>;
  readonly x_facebook_sources?: void;
  readonly x_facebook_segments?: FBSegmentMap;
  readonly x_hermes_function_offsets?: HermesFunctionOffsets;
  readonly x_google_ignoreList?: void;
}
export type MixedSourceMap = IndexMap | BasicSourceMap;
/**
 * Creates a source map from modules with "raw mappings", i.e. an array of
 * tuples with either 2, 4, or 5 elements:
 * generated line, generated column, source line, source line, symbol name.
 * Accepts an `offsetLines` argument in case modules' code is to be offset in
 * the resulting bundle, e.g. by some prefix code.
 */
declare function fromRawMappings(modules: ReadonlyArray<{
  readonly map?: null | ReadonlyArray<MetroSourceMapSegmentTuple>;
  readonly functionMap?: null | FBSourceFunctionMap;
  readonly path: string;
  readonly source: string;
  readonly code: string;
  readonly isIgnored: boolean;
  readonly lineCount?: number;
}>, offsetLines?: number): Generator;
declare function fromRawMappingsNonBlocking(modules: ReadonlyArray<{
  readonly map?: null | ReadonlyArray<MetroSourceMapSegmentTuple>;
  readonly functionMap?: null | FBSourceFunctionMap;
  readonly path: string;
  readonly source: string;
  readonly code: string;
  readonly isIgnored: boolean;
  readonly lineCount?: number;
}>, offsetLines?: number): Promise<Generator>;
/**
 * Transforms a standard source map object into a Raw Mappings object, to be
 * used across the bundler.
 */
declare function toBabelSegments(sourceMap: BasicSourceMap): Array<any>;
declare function toSegmentTuple(mapping: any): MetroSourceMapSegmentTuple;
export { BundleBuilder, composeSourceMaps, Consumer, createIndexMap, generateFunctionMap, fromRawMappings, fromRawMappingsNonBlocking, functionMapBabelPlugin, normalizeSourcePath, toBabelSegments, toSegmentTuple };
/**
 * Backwards-compatibility with CommonJS consumers using interopRequireDefault.
 * Do not add to this list.
 *
 * @deprecated Default import from 'metro-source-map' is deprecated, use named exports.
 */