/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 *
 * @format
 */

import type { HasteMapItem } from "../../flow-types";
export interface Conflict {
  id: string;
  platform?: string | null;
  absolutePaths: Array<string>;
  type?: "duplicate" | "shadowing";
}
export declare function computeHasteConflicts($$PARAM_0$$: {
  readonly duplicates: ReadonlyMap<string, ReadonlyMap<string, ReadonlyMap<string, number>>>;
  readonly map: ReadonlyMap<string, HasteMapItem>;
  readonly rootDir: string;
}): Array<Conflict>;