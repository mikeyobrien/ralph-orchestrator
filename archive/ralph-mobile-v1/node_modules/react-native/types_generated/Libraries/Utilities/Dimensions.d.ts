/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<fccdcec119b08f3650771d1058dcc362>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Utilities/Dimensions.js
 */

import { type EventSubscription } from "../vendor/emitter/EventEmitter";
import { type DimensionsPayload, type DisplayMetrics, type DisplayMetricsAndroid } from "./NativeDeviceInfo";
export type { DimensionsPayload, DisplayMetrics, DisplayMetricsAndroid };
/** @deprecated Use DisplayMetrics */
export type ScaledSize = DisplayMetrics;
declare class Dimensions {
  /**
   * NOTE: `useWindowDimensions` is the preferred API for React components.
   *
   * Initial dimensions are set before `runApplication` is called so they should
   * be available before any other require's are run, but may be updated later.
   *
   * Note: Although dimensions are available immediately, they may change (e.g
   * due to device rotation) so any rendering logic or styles that depend on
   * these constants should try to call this function on every render, rather
   * than caching the value (for example, using inline styles rather than
   * setting a value in a `StyleSheet`).
   *
   * Example: `const {height, width} = Dimensions.get('window');`
   *
   * @param {string} dim Name of dimension as defined when calling `set`.
   * @returns {DisplayMetrics? | DisplayMetricsAndroid?} Value for the dimension.
   */
  static get(dim: string): DisplayMetrics | DisplayMetricsAndroid;
  /**
   * This should only be called from native code by sending the
   * didUpdateDimensions event.
   *
   * @param {DimensionsPayload} dims Simple string-keyed object of dimensions to set
   */
  static set(dims: Readonly<DimensionsPayload>): void;
  /**
   * Add an event handler. Supported events:
   *
   * - `change`: Fires when a property within the `Dimensions` object changes. The argument
   *   to the event handler is an object with `window` and `screen` properties whose values
   *   are the same as the return values of `Dimensions.get('window')` and
   *   `Dimensions.get('screen')`, respectively.
   */
  static addEventListener(type: "change", handler: Function): EventSubscription;
}
export default Dimensions;
