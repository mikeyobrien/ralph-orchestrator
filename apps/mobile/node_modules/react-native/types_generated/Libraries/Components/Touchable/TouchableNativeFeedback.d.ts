/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<a9aee869154894b1a7e374c02786b866>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Components/Touchable/TouchableNativeFeedback.js
 */

import type { TouchableWithoutFeedbackProps } from "./TouchableWithoutFeedback";
import Pressability from "../../Pressability/Pressability";
import * as React from "react";
type TouchableNativeFeedbackTVProps = {
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
};
export type TouchableNativeFeedbackProps = Readonly<Omit<TouchableWithoutFeedbackProps, keyof TouchableNativeFeedbackTVProps | keyof {
  /**
   * Determines the type of background drawable that's going to be used to display feedback.
   * It takes an object with type property and extra data depending on the type.
   * It's recommended to use one of the following static methods to generate that dictionary:
   *      1) TouchableNativeFeedback.SelectableBackground() - will create object that represents android theme's
   *         default background for selectable elements (?android:attr/selectableItemBackground)
   *      2) TouchableNativeFeedback.SelectableBackgroundBorderless() - will create object that represent android
   *         theme's default background for borderless selectable elements
   *         (?android:attr/selectableItemBackgroundBorderless). Available on android API level 21+
   *      3) TouchableNativeFeedback.Ripple(color, borderless) - will create object that represents ripple drawable
   *         with specified color (as a string). If property borderless evaluates to true the ripple will render
   *         outside of the view bounds (see native actionbar buttons as an example of that behavior). This background
   *         type is available on Android API level 21+
   */
  background?: (Readonly<{
    type: "ThemeAttrAndroid";
    attribute: "selectableItemBackground" | "selectableItemBackgroundBorderless";
    rippleRadius: number | undefined;
  }> | Readonly<{
    type: "RippleAndroid";
    color: number | undefined;
    borderless: boolean;
    rippleRadius: number | undefined;
  }>) | undefined;
  /**
   * Set to true to add the ripple effect to the foreground of the view, instead
   * of the background. This is useful if one of your child views has a
   * background of its own, or you're e.g. displaying images, and you don't want
   * the ripple to be covered by them.
   *
   * Check TouchableNativeFeedback.canUseNativeForeground() first, as this is
   * only available on Android 6.0 and above. If you try to use this on older
   * versions, this will fallback to background.
   */
  useForeground?: boolean | undefined;
}> & Omit<TouchableNativeFeedbackTVProps, keyof {
  /**
   * Determines the type of background drawable that's going to be used to display feedback.
   * It takes an object with type property and extra data depending on the type.
   * It's recommended to use one of the following static methods to generate that dictionary:
   *      1) TouchableNativeFeedback.SelectableBackground() - will create object that represents android theme's
   *         default background for selectable elements (?android:attr/selectableItemBackground)
   *      2) TouchableNativeFeedback.SelectableBackgroundBorderless() - will create object that represent android
   *         theme's default background for borderless selectable elements
   *         (?android:attr/selectableItemBackgroundBorderless). Available on android API level 21+
   *      3) TouchableNativeFeedback.Ripple(color, borderless) - will create object that represents ripple drawable
   *         with specified color (as a string). If property borderless evaluates to true the ripple will render
   *         outside of the view bounds (see native actionbar buttons as an example of that behavior). This background
   *         type is available on Android API level 21+
   */
  background?: (Readonly<{
    type: "ThemeAttrAndroid";
    attribute: "selectableItemBackground" | "selectableItemBackgroundBorderless";
    rippleRadius: number | undefined;
  }> | Readonly<{
    type: "RippleAndroid";
    color: number | undefined;
    borderless: boolean;
    rippleRadius: number | undefined;
  }>) | undefined;
  /**
   * Set to true to add the ripple effect to the foreground of the view, instead
   * of the background. This is useful if one of your child views has a
   * background of its own, or you're e.g. displaying images, and you don't want
   * the ripple to be covered by them.
   *
   * Check TouchableNativeFeedback.canUseNativeForeground() first, as this is
   * only available on Android 6.0 and above. If you try to use this on older
   * versions, this will fallback to background.
   */
  useForeground?: boolean | undefined;
}> & {
  /**
   * Determines the type of background drawable that's going to be used to display feedback.
   * It takes an object with type property and extra data depending on the type.
   * It's recommended to use one of the following static methods to generate that dictionary:
   *      1) TouchableNativeFeedback.SelectableBackground() - will create object that represents android theme's
   *         default background for selectable elements (?android:attr/selectableItemBackground)
   *      2) TouchableNativeFeedback.SelectableBackgroundBorderless() - will create object that represent android
   *         theme's default background for borderless selectable elements
   *         (?android:attr/selectableItemBackgroundBorderless). Available on android API level 21+
   *      3) TouchableNativeFeedback.Ripple(color, borderless) - will create object that represents ripple drawable
   *         with specified color (as a string). If property borderless evaluates to true the ripple will render
   *         outside of the view bounds (see native actionbar buttons as an example of that behavior). This background
   *         type is available on Android API level 21+
   */
  background?: (Readonly<{
    type: "ThemeAttrAndroid";
    attribute: "selectableItemBackground" | "selectableItemBackgroundBorderless";
    rippleRadius: number | undefined;
  }> | Readonly<{
    type: "RippleAndroid";
    color: number | undefined;
    borderless: boolean;
    rippleRadius: number | undefined;
  }>) | undefined;
  /**
   * Set to true to add the ripple effect to the foreground of the view, instead
   * of the background. This is useful if one of your child views has a
   * background of its own, or you're e.g. displaying images, and you don't want
   * the ripple to be covered by them.
   *
   * Check TouchableNativeFeedback.canUseNativeForeground() first, as this is
   * only available on Android 6.0 and above. If you try to use this on older
   * versions, this will fallback to background.
   */
  useForeground?: boolean | undefined;
}>;
type TouchableNativeFeedbackState = Readonly<{
  pressability: Pressability;
}>;
/**
 * A wrapper for making views respond properly to touches (Android only).
 * On Android this component uses native state drawable to display touch feedback.
 * At the moment it only supports having a single View instance as a child node,
 * as it's implemented by replacing that View with another instance of RCTView node with some additional properties set.
 *
 * Background drawable of native feedback touchable can be customized with background property.
 *
 * @see https://reactnative.dev/docs/touchablenativefeedback#content
 */
declare class TouchableNativeFeedback extends React.Component<TouchableNativeFeedbackProps, TouchableNativeFeedbackState> {
  /**
   * Creates an object that represents android theme's default background for
   * selectable elements (?android:attr/selectableItemBackground).
   *
   * @param rippleRadius The radius of ripple effect
   */
  static SelectableBackground: (rippleRadius?: null | undefined | number) => Readonly<{
    attribute: "selectableItemBackground";
    type: "ThemeAttrAndroid";
    rippleRadius: number | undefined;
  }>;
  /**
   * Creates an object that represent android theme's default background for borderless
   * selectable elements (?android:attr/selectableItemBackgroundBorderless).
   * Available on android API level 21+.
   *
   * @param rippleRadius The radius of ripple effect
   */
  static SelectableBackgroundBorderless: (rippleRadius?: null | undefined | number) => Readonly<{
    attribute: "selectableItemBackgroundBorderless";
    type: "ThemeAttrAndroid";
    rippleRadius: number | undefined;
  }>;
  /**
   * Creates an object that represents ripple drawable with specified color (as a
   * string). If property `borderless` evaluates to true the ripple will
   * render outside of the view bounds (see native actionbar buttons as an
   * example of that behavior). This background type is available on Android
   * API level 21+.
   *
   * @param color The ripple color
   * @param borderless If the ripple can render outside it's bounds
   * @param rippleRadius The radius of ripple effect
   */
  static Ripple: (color: string, borderless: boolean, rippleRadius?: null | undefined | number) => Readonly<{
    borderless: boolean;
    color: number | undefined;
    rippleRadius: number | undefined;
    type: "RippleAndroid";
  }>;
  /**
   * Whether `useForeground` is supported.
   */
  static canUseNativeForeground: () => boolean;
  state: TouchableNativeFeedbackState;
  render(): React.ReactNode;
  componentDidUpdate(prevProps: TouchableNativeFeedbackProps, prevState: TouchableNativeFeedbackState): void;
  componentDidMount(): unknown;
  componentWillUnmount(): void;
}
export default TouchableNativeFeedback;
