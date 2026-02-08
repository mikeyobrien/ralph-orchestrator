/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<c677afc34af685572670d7d4065a70dc>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/virtualized-lists/Lists/VirtualizeUtils.js
 */

import type ListMetricsAggregator from "./ListMetricsAggregator";
import type { CellMetricProps } from "./ListMetricsAggregator";
/**
 * Used to find the indices of the frames that overlap the given offsets. Useful for finding the
 * items that bound different windows of content, such as the visible area or the buffered overscan
 * area.
 */
export declare function elementsThatOverlapOffsets(offsets: Array<number>, props: CellMetricProps, listMetrics: ListMetricsAggregator, zoomScale?: number): Array<number>;
/**
 * Computes the number of elements in the `next` range that are new compared to the `prev` range.
 * Handy for calculating how many new items will be rendered when the render window changes so we
 * can restrict the number of new items render at once so that content can appear on the screen
 * faster.
 */
export declare function newRangeCount(prev: {
  first: number;
  last: number;
}, next: {
  first: number;
  last: number;
}): number;
/**
 * Custom logic for determining which items should be rendered given the current frame and scroll
 * metrics, as well as the previous render state. The algorithm may evolve over time, but generally
 * prioritizes the visible area first, then expands that with overscan regions ahead and behind,
 * biased in the direction of scroll.
 */
export declare function computeWindowedRenderLimits(props: CellMetricProps, maxToRenderPerBatch: number, windowSize: number, prev: {
  first: number;
  last: number;
}, listMetrics: ListMetricsAggregator, scrollMetrics: {
  dt: number;
  offset: number;
  velocity: number;
  visibleLength: number;
  zoomScale: number;
}): {
  first: number;
  last: number;
};
export declare function keyExtractor(item: any, index: number): string;
