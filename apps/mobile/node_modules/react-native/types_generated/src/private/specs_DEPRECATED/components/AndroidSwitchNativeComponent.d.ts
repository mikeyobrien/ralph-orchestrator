/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<744203266059e3218a3624a844e40408>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/src/private/specs_DEPRECATED/components/AndroidSwitchNativeComponent.js
 */

import type { ViewProps } from "../../../../Libraries/Components/View/ViewPropTypes";
import type { ColorValue } from "../../../../Libraries/StyleSheet/StyleSheet";
import type { BubblingEventHandler, Int32, WithDefault } from "../../../../Libraries/Types/CodegenTypes";
import type { HostComponent } from "../../types/HostComponent";
import * as React from "react";
type AndroidSwitchChangeEvent = Readonly<{
  value: boolean;
  target: Int32;
}>;
type AndroidSwitchNativeProps = Readonly<Omit<ViewProps, keyof {
  disabled?: WithDefault<boolean, false>;
  enabled?: WithDefault<boolean, true>;
  thumbColor?: ColorValue | undefined;
  trackColorForFalse?: ColorValue | undefined;
  trackColorForTrue?: ColorValue | undefined;
  value?: WithDefault<boolean, false>;
  on?: WithDefault<boolean, false>;
  thumbTintColor?: ColorValue | undefined;
  trackTintColor?: ColorValue | undefined;
  onChange?: BubblingEventHandler<AndroidSwitchChangeEvent>;
}> & {
  disabled?: WithDefault<boolean, false>;
  enabled?: WithDefault<boolean, true>;
  thumbColor?: ColorValue | undefined;
  trackColorForFalse?: ColorValue | undefined;
  trackColorForTrue?: ColorValue | undefined;
  value?: WithDefault<boolean, false>;
  on?: WithDefault<boolean, false>;
  thumbTintColor?: ColorValue | undefined;
  trackTintColor?: ColorValue | undefined;
  onChange?: BubblingEventHandler<AndroidSwitchChangeEvent>;
}>;
type NativeType = HostComponent<AndroidSwitchNativeProps>;
interface NativeCommands {
  readonly setNativeValue: (viewRef: React.ComponentRef<NativeType>, value: boolean) => void;
}
export declare const Commands: NativeCommands;
export declare type Commands = typeof Commands;
declare const $$AndroidSwitchNativeComponent: NativeType;
declare type $$AndroidSwitchNativeComponent = typeof $$AndroidSwitchNativeComponent;
export default $$AndroidSwitchNativeComponent;
