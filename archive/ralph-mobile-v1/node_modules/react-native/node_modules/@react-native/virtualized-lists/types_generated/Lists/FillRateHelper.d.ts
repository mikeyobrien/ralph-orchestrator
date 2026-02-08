/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<e813b2d289d62a261179b333cb431e6f>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/virtualized-lists/Lists/FillRateHelper.js
 */

import type { CellMetricProps } from "./ListMetricsAggregator";
import ListMetricsAggregator from "./ListMetricsAggregator";
export type FillRateInfo = Info;
declare class Info {
  any_blank_count: number;
  any_blank_ms: number;
  any_blank_speed_sum: number;
  mostly_blank_count: number;
  mostly_blank_ms: number;
  pixels_blank: number;
  pixels_sampled: number;
  pixels_scrolled: number;
  total_time_spent: number;
  sample_count: number;
}
/**
 * A helper class for detecting when the maximem fill rate of `VirtualizedList` is exceeded.
 * By default the sampling rate is set to zero and this will do nothing. If you want to collect
 * samples (e.g. to log them), make sure to call `FillRateHelper.setSampleRate(0.0-1.0)`.
 *
 * Listeners and sample rate are global for all `VirtualizedList`s - typical usage will combine with
 * `SceneTracker.getActiveScene` to determine the context of the events.
 */
declare class FillRateHelper {
  static addListener(callback: ($$PARAM_0$$: FillRateInfo) => void): {
    remove: () => void;
  };
  static setSampleRate(sampleRate: number): void;
  static setMinSampleCount(minSampleCount: number): void;
  constructor(listMetrics: ListMetricsAggregator);
  activate(): void;
  deactivateAndFlush(): void;
  computeBlankness(props: Omit<CellMetricProps, keyof {
    initialNumToRender?: number | undefined;
  }> & {
    initialNumToRender?: number | undefined;
  }, cellsAroundViewport: {
    first: number;
    last: number;
  }, scrollMetrics: {
    dOffset: number;
    offset: number;
    velocity: number;
    visibleLength: number;
  }): number;
  enabled(): boolean;
}
export default FillRateHelper;
