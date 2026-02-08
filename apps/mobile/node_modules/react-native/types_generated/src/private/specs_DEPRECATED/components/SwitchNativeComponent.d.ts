/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<750316f22a9730a31f3609f6b33e1d04>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/src/private/specs_DEPRECATED/components/SwitchNativeComponent.js
 */

import type { ViewProps } from "../../../../Libraries/Components/View/ViewPropTypes";
import type { ColorValue } from "../../../../Libraries/StyleSheet/StyleSheet";
import type { BubblingEventHandler, Int32, WithDefault } from "../../../../Libraries/Types/CodegenTypes";
import type { HostComponent } from "../../types/HostComponent";
import * as React from "react";
type NativeSwitchChangeEvent = Readonly<{
  value: boolean;
  target: Int32;
}>;
type SwitchNativeProps = Readonly<Omit<ViewProps, keyof {
  disabled?: WithDefault<boolean, false>;
  value?: WithDefault<boolean, false>;
  tintColor?: ColorValue | undefined;
  onTintColor?: ColorValue | undefined;
  thumbTintColor?: ColorValue | undefined;
  thumbColor?: ColorValue | undefined;
  trackColorForFalse?: ColorValue | undefined;
  trackColorForTrue?: ColorValue | undefined;
  onChange?: BubblingEventHandler<NativeSwitchChangeEvent> | undefined;
}> & {
  disabled?: WithDefault<boolean, false>;
  value?: WithDefault<boolean, false>;
  tintColor?: ColorValue | undefined;
  onTintColor?: ColorValue | undefined;
  thumbTintColor?: ColorValue | undefined;
  thumbColor?: ColorValue | undefined;
  trackColorForFalse?: ColorValue | undefined;
  trackColorForTrue?: ColorValue | undefined;
  onChange?: BubblingEventHandler<NativeSwitchChangeEvent> | undefined;
}>;
type ComponentType = HostComponent<SwitchNativeProps>;
interface NativeCommands {
  readonly setValue: (viewRef: React.ComponentRef<ComponentType>, value: boolean) => void;
}
export declare const Commands: NativeCommands;
export declare type Commands = typeof Commands;
declare const $$SwitchNativeComponent: ComponentType;
declare type $$SwitchNativeComponent = typeof $$SwitchNativeComponent;
export default $$SwitchNativeComponent;
