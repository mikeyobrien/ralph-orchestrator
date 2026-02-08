/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<c71c6155a6c1167093da67c32c306d07>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/virtualized-lists/Lists/ViewabilityHelper.js
 */

import type { CellMetricProps } from "./ListMetricsAggregator";
import ListMetricsAggregator from "./ListMetricsAggregator";
export type ViewToken = {
  item: any;
  key: string;
  index: number | undefined;
  isViewable: boolean;
  section?: any;
};
export type ViewabilityConfigCallbackPair = {
  viewabilityConfig: ViewabilityConfig;
  onViewableItemsChanged: (info: {
    viewableItems: Array<ViewToken>;
    changed: Array<ViewToken>;
  }) => void;
};
export type ViewabilityConfigCallbackPairs = Array<ViewabilityConfigCallbackPair>;
export type ViewabilityConfig = Readonly<{
  /**
   * Minimum amount of time (in milliseconds) that an item must be physically viewable before the
   * viewability callback will be fired. A high number means that scrolling through content without
   * stopping will not mark the content as viewable.
   */
  minimumViewTime?: number;
  /**
   * Percent of viewport that must be covered for a partially occluded item to count as
   * "viewable", 0-100. Fully visible items are always considered viewable. A value of 0 means
   * that a single pixel in the viewport makes the item viewable, and a value of 100 means that
   * an item must be either entirely visible or cover the entire viewport to count as viewable.
   */
  viewAreaCoveragePercentThreshold?: number;
  /**
   * Similar to `viewAreaPercentThreshold`, but considers the percent of the item that is visible,
   * rather than the fraction of the viewable area it covers.
   */
  itemVisiblePercentThreshold?: number;
  /**
   * Nothing is considered viewable until the user scrolls or `recordInteraction` is called after
   * render.
   */
  waitForInteraction?: boolean;
}>;
/**
 * A Utility class for calculating viewable items based on current metrics like scroll position and
 * layout.
 *
 * An item is said to be in a "viewable" state when any of the following
 * is true for longer than `minimumViewTime` milliseconds (after an interaction if `waitForInteraction`
 * is true):
 *
 * - Occupying >= `viewAreaCoveragePercentThreshold` of the view area XOR fraction of the item
 *   visible in the view area >= `itemVisiblePercentThreshold`.
 * - Entirely visible on screen
 */
declare class ViewabilityHelper {
  constructor(config?: ViewabilityConfig);
  /**
   * Cleanup, e.g. on unmount. Clears any pending timers.
   */
  dispose(): void;
  /**
   * Determines which items are viewable based on the current metrics and config.
   */
  computeViewableItems(props: CellMetricProps, scrollOffset: number, viewportHeight: number, listMetrics: ListMetricsAggregator, renderRange?: {
    first: number;
    last: number;
  }): Array<number>;
  /**
   * Figures out which items are viewable and how that has changed from before and calls
   * `onViewableItemsChanged` as appropriate.
   */
  onUpdate(props: CellMetricProps, scrollOffset: number, viewportHeight: number, listMetrics: ListMetricsAggregator, createViewToken: (index: number, isViewable: boolean, props: CellMetricProps) => ViewToken, onViewableItemsChanged: ($$PARAM_0$$: {
    viewableItems: Array<ViewToken>;
    changed: Array<ViewToken>;
  }) => void, renderRange?: {
    first: number;
    last: number;
  }): void;
  /**
   * clean-up cached _viewableIndices to evaluate changed items on next update
   */
  resetViewableIndices(): void;
  /**
   * Records that an interaction has happened even if there has been no scroll.
   */
  recordInteraction(): void;
}
export default ViewabilityHelper;
