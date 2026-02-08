/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<baeebf19166453f04a30602fccb951e7>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Components/Touchable/TouchableOpacity.js
 */

import type { ViewStyleProp } from "../../StyleSheet/StyleSheet";
import type { TouchableWithoutFeedbackProps } from "./TouchableWithoutFeedback";
import Animated from "../../Animated/Animated";
import * as React from "react";
export type TouchableOpacityTVProps = Readonly<{
  /**
   * *(Apple TV only)* TV preferred focus (see documentation for the View component).
   *
   * @platform ios
   * @deprecated Use `focusable` instead
   */
  hasTVPreferredFocus?: boolean | undefined;
  /**
   * Designates the next view to receive focus when the user navigates down. See the Android documentation.
   *
   * @platform android
   */
  nextFocusDown?: number | undefined;
  /**
   * Designates the next view to receive focus when the user navigates forward. See the Android documentation.
   *
   * @platform android
   */
  nextFocusForward?: number | undefined;
  /**
   * Designates the next view to receive focus when the user navigates left. See the Android documentation.
   *
   * @platform android
   */
  nextFocusLeft?: number | undefined;
  /**
   * Designates the next view to receive focus when the user navigates right. See the Android documentation.
   *
   * @platform android
   */
  nextFocusRight?: number | undefined;
  /**
   * Designates the next view to receive focus when the user navigates up. See the Android documentation.
   *
   * @platform android
   */
  nextFocusUp?: number | undefined;
}>;
type TouchableOpacityBaseProps = Readonly<{
  /**
   * Determines what the opacity of the wrapped view should be when touch is active.
   * Defaults to 0.2
   */
  activeOpacity?: number | undefined;
  style?: Animated.WithAnimatedValue<ViewStyleProp> | undefined;
  hostRef?: React.Ref<React.ComponentRef<typeof Animated.View>> | undefined;
}>;
export type TouchableOpacityProps = Readonly<Omit<TouchableWithoutFeedbackProps, keyof TouchableOpacityTVProps | keyof TouchableOpacityBaseProps | keyof {}> & Omit<TouchableOpacityTVProps, keyof TouchableOpacityBaseProps | keyof {}> & Omit<TouchableOpacityBaseProps, keyof {}> & {}>;
declare const Touchable: (props: Omit<TouchableOpacityProps, keyof {
  ref?: React.Ref<React.ComponentRef<typeof Animated.View>>;
}> & {
  ref?: React.Ref<React.ComponentRef<typeof Animated.View>>;
}) => React.ReactNode;
declare const $$TouchableOpacity: typeof Touchable;
declare type $$TouchableOpacity = typeof $$TouchableOpacity;
export default $$TouchableOpacity;
