/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<f61be1758ae0ac5c3b456e1351e93910>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/virtualized-lists/Lists/CellRenderMask.js
 */

export type CellRegion = {
  first: number;
  last: number;
  isSpacer: boolean;
};
export declare class CellRenderMask {
  constructor(numCells: number);
  enumerateRegions(): ReadonlyArray<CellRegion>;
  addCells(cells: {
    first: number;
    last: number;
  }): void;
  numCells(): number;
  equals(other: CellRenderMask): boolean;
}
