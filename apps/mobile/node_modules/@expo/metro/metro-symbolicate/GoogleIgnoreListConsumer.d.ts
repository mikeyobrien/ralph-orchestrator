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

import type { MixedSourceMap } from "../metro-source-map";
type SourceNameNormalizer = ($$PARAM_0$$: string, $$PARAM_1$$: {
  readonly sourceRoot?: null | undefined | string;
}) => string;
/**
  * Consumes the `x_google_ignoreList` metadata field from a source map and
  * exposes various queries on it.
  *
  * By default, source names are normalized using the same logic that the
  * `source-map@0.5.6` package uses internally. This is crucial for keeping the
  * sources list in sync with a `SourceMapConsumer` instance.

  * If you're using this with a different source map reader (e.g. one that
  * doesn't normalize source names at all), you can switch out the normalization
  * function in the constructor, e.g.
  *
  *     new GoogleIgnoreListConsumer(map, source => source) // Don't normalize
  */
declare class GoogleIgnoreListConsumer {
  constructor(map: MixedSourceMap, normalizeSourceFn: SourceNameNormalizer);
  _sourceMap: MixedSourceMap;
  _normalizeSource: SourceNameNormalizer;
  _ignoredSourceSet: null | undefined | Set<string>;
  isIgnored($$PARAM_0$$: {
    readonly source?: null | string;
  }): boolean;
  toArray(sources: ReadonlyArray<null | undefined | string>): Array<number>;
  _getIgnoredSourceSet(): ReadonlySet<string>;
  _buildIgnoredSourceSet(map: MixedSourceMap, ignoredSourceSet: Set<string>): void;
}
export default GoogleIgnoreListConsumer;