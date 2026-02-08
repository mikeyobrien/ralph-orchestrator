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

import type { Module } from "../../types";
declare function processModules(modules: ReadonlyArray<Module>, $$PARAM_1$$: {
  readonly filter?: (module: Module) => boolean;
  readonly createModuleId: ($$PARAM_0$$: string) => number;
  readonly dev: boolean;
  readonly includeAsyncPaths: boolean;
  readonly projectRoot: string;
  readonly serverRoot: string;
  readonly sourceUrl?: null | string;
}): ReadonlyArray<[Module, string]>;
export default processModules;