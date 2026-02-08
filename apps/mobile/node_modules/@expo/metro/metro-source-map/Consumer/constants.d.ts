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

declare const FIRST_COLUMN: number;
declare const FIRST_LINE: number;
export declare type IterationOrder = unknown;
declare const GENERATED_ORDER: IterationOrder;
declare const ORIGINAL_ORDER: IterationOrder;
export declare type LookupBias = unknown;
declare const GREATEST_LOWER_BOUND: LookupBias;
declare const LEAST_UPPER_BOUND: LookupBias;
declare const EMPTY_POSITION: {
  readonly source: null;
  readonly name: null;
  readonly line: null;
  readonly column: null;
};
declare function iterationOrderToString(x: IterationOrder): string;
declare function lookupBiasToString(x: LookupBias): string;
export { FIRST_COLUMN, FIRST_LINE, GENERATED_ORDER, ORIGINAL_ORDER, GREATEST_LOWER_BOUND, LEAST_UPPER_BOUND, EMPTY_POSITION, iterationOrderToString, lookupBiasToString };