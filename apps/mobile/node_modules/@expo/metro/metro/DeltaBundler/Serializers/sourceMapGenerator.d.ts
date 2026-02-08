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

import type { Module } from "../types";
import { fromRawMappings, fromRawMappingsNonBlocking } from "../../../metro-source-map";
export interface SourceMapGeneratorOptions {
  readonly excludeSource: boolean;
  readonly processModuleFilter: (module: Module) => boolean;
  readonly shouldAddToIgnoreList: (module: Module) => boolean;
  readonly getSourceUrl?: null | ((module: Module) => string);
}
declare function sourceMapGenerator(modules: ReadonlyArray<Module>, options: SourceMapGeneratorOptions): ReturnType<typeof fromRawMappings>;
declare function sourceMapGeneratorNonBlocking(modules: ReadonlyArray<Module>, options: SourceMapGeneratorOptions): ReturnType<typeof fromRawMappingsNonBlocking>;
export { sourceMapGenerator, sourceMapGeneratorNonBlocking };