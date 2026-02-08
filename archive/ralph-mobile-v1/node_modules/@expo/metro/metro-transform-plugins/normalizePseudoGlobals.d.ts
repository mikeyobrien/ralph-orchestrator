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

import type { Node } from "@babel/types";
export interface Options {
  reservedNames: ReadonlyArray<string>;
}
declare function normalizePseudoglobals(ast: Node, options?: Options): ReadonlyArray<string>;
export default normalizePseudoglobals;