/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<993b116aaf2c8d4835bce3f9a4c6392f>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/virtualized-lists/Lists/ListMetricsAggregator.js
 */

import type { VirtualizedListProps } from "./VirtualizedListProps";
import type { LayoutRectangle } from "react-native";
export type CellMetrics = {
  /**
   * Index of the item in the list
   */
  index: number;
  /**
   * Length of the cell along the scrolling axis
   */
  length: number;
  /**
   * Distance between this cell and the start of the list along the scrolling
   * axis
   */
  offset: number;
  /**
   * Whether the cell is last known to be mounted
   */
  isMounted: boolean;
};
export type ListOrientation = {
  horizontal: boolean;
  rtl: boolean;
};
/**
 * Subset of VirtualizedList props needed to calculate cell metrics
 */
export type CellMetricProps = {
  data: VirtualizedListProps["data"];
  getItemCount: VirtualizedListProps["getItemCount"];
  getItem: VirtualizedListProps["getItem"];
  getItemLayout?: VirtualizedListProps["getItemLayout"];
  keyExtractor?: VirtualizedListProps["keyExtractor"];
};
/**
 * Provides an interface to query information about the metrics of a list and its cells.
 */
declare class ListMetricsAggregator {
  /**
   * Notify the ListMetricsAggregator that a cell has been laid out.
   *
   * @returns whether the cell layout has changed since last notification
   */
  notifyCellLayout($$PARAM_0$$: {
    cellIndex: number;
    cellKey: string;
    orientation: ListOrientation;
    layout: LayoutRectangle;
  }): boolean;
  /**
   * Notify ListMetricsAggregator that a cell has been unmounted.
   */
  notifyCellUnmounted(cellKey: string): void;
  /**
   * Notify ListMetricsAggregator that the lists content container has been laid out.
   */
  notifyListContentLayout($$PARAM_0$$: {
    orientation: ListOrientation;
    layout: Readonly<{
      width: number;
      height: number;
    }>;
  }): void;
  /**
   * Return the average length of the cells which have been measured
   */
  getAverageCellLength(): number;
  /**
   * Return the highest measured cell index (or 0 if nothing has been measured
   * yet)
   */
  getHighestMeasuredCellIndex(): number;
  /**
   * Returns the exact metrics of a cell if it has already been laid out,
   * otherwise an estimate based on the average length of previously measured
   * cells
   */
  getCellMetricsApprox(index: number, props: CellMetricProps): CellMetrics;
  /**
   * Returns the exact metrics of a cell if it has already been laid out
   */
  getCellMetrics(index: number, props: CellMetricProps): null | undefined | CellMetrics;
  /**
   * Gets an approximate offset to an item at a given index. Supports
   * fractional indices.
   */
  getCellOffsetApprox(index: number, props: CellMetricProps): number;
  /**
   * Returns the length of all ScrollView content along the scrolling axis.
   */
  getContentLength(): number;
  /**
   * Whether a content length has been observed
   */
  hasContentLength(): boolean;
  /**
   * Finds the flow-relative offset (e.g. starting from the left in LTR, but
   * right in RTL) from a layout box.
   */
  flowRelativeOffset(layout: LayoutRectangle, referenceContentLength?: null | undefined | number): number;
  /**
   * Converts a flow-relative offset to a cartesian offset
   */
  cartesianOffset(flowRelativeOffset: number): number;
}
export default ListMetricsAggregator;
