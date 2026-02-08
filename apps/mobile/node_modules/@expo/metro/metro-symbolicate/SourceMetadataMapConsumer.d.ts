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

import type { FBSourceMetadata, FBSourcesArray, MixedSourceMap } from "../metro-source-map";
export interface Position {
  readonly line: number;
  readonly column: number;
}
export interface FunctionMapping {
  readonly line: number;
  readonly column: number;
  readonly name: string;
}
type SourceNameNormalizer = ($$PARAM_0$$: string, $$PARAM_1$$: {
  readonly sourceRoot?: null | undefined | string;
}) => string;
export interface MetadataMap {
  [source: string]: null | undefined | FBSourceMetadata;
}
/**
 * Consumes the `x_facebook_sources` metadata field from a source map and
 * exposes various queries on it.
 *
 * By default, source names are normalized using the same logic that the
 * `source-map@0.5.6` package uses internally. This is crucial for keeping the
 * sources list in sync with a `SourceMapConsumer` instance.

 * If you're using this with a different source map reader (e.g. one that
 * doesn't normalize source names at all), you can switch out the normalization
 * function in the constructor, e.g.
 *
 *     new SourceMetadataMapConsumer(map, source => source) // Don't normalize
 */
declare class SourceMetadataMapConsumer {
  constructor(map: MixedSourceMap, normalizeSourceFn: SourceNameNormalizer);
  _sourceMap: MixedSourceMap;
  _decodedFunctionMapCache: Map<string, null | undefined | ReadonlyArray<FunctionMapping>>;
  _normalizeSource: SourceNameNormalizer;
  _metadataBySource: null | undefined | MetadataMap;
  functionNameFor($$PARAM_0$$: Position & {
    readonly source?: null | string;
  }): null | undefined | string;
  toArray(sources: ReadonlyArray<string>): FBSourcesArray;
  _getMetadataBySource(): MetadataMap;
  _getFunctionMappings(source: string): null | undefined | ReadonlyArray<FunctionMapping>;
  _getMetadataObjectsBySourceNames(map: MixedSourceMap): MetadataMap;
}
export default SourceMetadataMapConsumer;