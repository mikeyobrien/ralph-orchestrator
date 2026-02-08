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

import type { PluginObj } from "@babel/core";
import type { Node, Statement, SourceLocation } from "@babel/types";
import * as Types from "@babel/types";
export interface _Options_out {
  isESModule: boolean;
}
export interface Options {
  readonly importDefault: string;
  readonly importAll: string;
  readonly resolve: boolean;
  readonly out?: _Options_out;
}
export interface State {
  exportAll: Array<{
    file: string;
    loc?: null | SourceLocation;
  }>;
  exportDefault: Array<{
    local: string;
    loc?: null | SourceLocation;
  }>;
  exportNamed: Array<{
    local: string;
    remote: string;
    loc?: null | SourceLocation;
  }>;
  imports: Array<{
    node: Statement;
  }>;
  importDefault: Node;
  importAll: Node;
  opts: Options;
}
declare function importExportPlugin($$PARAM_0$$: {
  types: typeof Types;
}): PluginObj<State>;
export default importExportPlugin;