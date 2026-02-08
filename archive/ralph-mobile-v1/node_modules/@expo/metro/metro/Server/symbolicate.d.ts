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

import type { ExplodedSourceMap } from "../DeltaBundler/Serializers/getExplodedSourceMap";
import type { ConfigT } from "../../metro-config";
export interface StackFrameInput {
  readonly file?: null | string;
  readonly lineNumber?: null | number;
  readonly column?: null | number;
  readonly methodName?: null | string;
}
export interface IntermediateStackFrame extends StackFrameInput {
  collapse?: boolean;
}
export type StackFrameOutput = Readonly<IntermediateStackFrame>;
declare function symbolicate(stack: ReadonlyArray<StackFrameInput>, maps: Iterable<[string, ExplodedSourceMap]>, config: ConfigT, extraData: any): Promise<ReadonlyArray<StackFrameOutput>>;
export default symbolicate;