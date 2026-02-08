/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<55bd8debf6bc6068e4368f5b7905b017>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/virtualized-lists/Lists/VirtualizedListContext.js
 */

import type $$IMPORT_TYPEOF_1$$ from "./VirtualizedList";
type VirtualizedListT = typeof $$IMPORT_TYPEOF_1$$;
import * as React from "react";
type Context = Readonly<{
  cellKey: string | undefined;
  getScrollMetrics: () => {
    contentLength: number;
    dOffset: number;
    dt: number;
    offset: number;
    timestamp: number;
    velocity: number;
    visibleLength: number;
    zoomScale: number;
  };
  horizontal: boolean | undefined;
  getOutermostParentListRef: () => React.ComponentRef<VirtualizedListT>;
  registerAsNestedChild: ($$PARAM_0$$: {
    cellKey: string;
    ref: React.ComponentRef<VirtualizedListT>;
  }) => void;
  unregisterAsNestedChild: ($$PARAM_0$$: {
    ref: React.ComponentRef<VirtualizedListT>;
  }) => void;
}>;
export declare const VirtualizedListContext: React.Context<null | undefined | Context>;
export declare type VirtualizedListContext = typeof VirtualizedListContext;
/**
 * Resets the context. Intended for use by portal-like components (e.g. Modal).
 */
export declare function VirtualizedListContextResetter($$PARAM_0$$: {
  children: React.ReactNode;
}): React.ReactNode;
/**
 * Sets the context with memoization. Intended to be used by `VirtualizedList`.
 */
export declare function VirtualizedListContextProvider($$PARAM_0$$: {
  children: React.ReactNode;
  value: Context;
}): React.ReactNode;
/**
 * Sets the `cellKey`. Intended to be used by `VirtualizedList` for each cell.
 */
export declare function VirtualizedListCellContextProvider($$PARAM_0$$: {
  cellKey: string;
  children: React.ReactNode;
}): React.ReactNode;
