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

import type { MixedOutput, Module } from "../../types";
import type { JsOutput } from "../../../../metro-transform-worker";
export interface Options {
  readonly createModuleId: ($$PARAM_0$$: string) => number | string;
  readonly dev: boolean;
  readonly includeAsyncPaths: boolean;
  readonly projectRoot: string;
  readonly serverRoot: string;
  readonly sourceUrl?: null | string;
}
export declare function wrapModule(module: Module, options: Options): string;
export declare function getModuleParams(module: Module, options: Options): Array<any>;
export declare function getJsOutput(module: {
  readonly output: ReadonlyArray<MixedOutput>;
  readonly path?: string;
}): JsOutput;
export declare function isJsModule(module: Module): boolean;