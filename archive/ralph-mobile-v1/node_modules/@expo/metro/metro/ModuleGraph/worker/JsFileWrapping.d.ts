/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 *
 */

import type { File } from "@babel/types";
declare const WRAP_NAME: "$$_REQUIRE";
declare function wrapModule(fileAst: File, importDefaultName: string, importAllName: string, dependencyMapName: string, globalPrefix: string, skipRequireRename: boolean, _optionalArg?: {
  readonly unstable_useStaticHermesModuleFactory?: boolean;
}): {
  ast: File;
  requireName: string;
};
declare function wrapPolyfill(fileAst: File): File;
declare function jsonToCommonJS(source: string): string;
declare function wrapJson(source: string, globalPrefix: string, unstable_useStaticHermesModuleFactory?: boolean): string;
export { WRAP_NAME, wrapJson, jsonToCommonJS, wrapModule, wrapPolyfill };