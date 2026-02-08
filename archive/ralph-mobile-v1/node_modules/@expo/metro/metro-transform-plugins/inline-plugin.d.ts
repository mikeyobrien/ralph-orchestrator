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
import * as Types from "@babel/types";
export interface Options {
  readonly dev: boolean;
  readonly inlinePlatform: boolean;
  readonly isWrapped: boolean;
  readonly requireName?: string;
  readonly platform: string;
}
export interface State {
  opts: Options;
}
declare function inlinePlugin($$PARAM_0$$: {
  types: typeof Types;
}, options: Options): PluginObj<State>;
export default inlinePlugin;