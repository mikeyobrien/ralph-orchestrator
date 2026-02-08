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

import type { PluginObj, BabelFile } from "@babel/core";
import * as Types from "@babel/types";
type ImportDeclarationLocs = Set<string>;
export interface State {
  importDeclarationLocs: ImportDeclarationLocs;
  file: BabelFile;
}
declare function importLocationsPlugin($$PARAM_0$$: {
  types: typeof Types;
}): PluginObj<State>;
declare function locToKey(loc: Types.SourceLocation): string;
export { importLocationsPlugin, locToKey };