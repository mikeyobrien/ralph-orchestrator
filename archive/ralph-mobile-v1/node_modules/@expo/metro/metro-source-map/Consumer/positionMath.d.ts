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

import type { GeneratedOffset } from "./types";
export declare function shiftPositionByOffset<T extends {
  readonly line?: null | number;
  readonly column?: null | number;
}>(pos: T, offset: GeneratedOffset): T;
export declare function subtractOffsetFromPosition<T extends {
  readonly line?: null | number;
  readonly column?: null | number;
}>(pos: T, offset: GeneratedOffset): T;