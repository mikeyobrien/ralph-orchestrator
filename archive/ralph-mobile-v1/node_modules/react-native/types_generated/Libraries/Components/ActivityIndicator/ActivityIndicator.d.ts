/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<a4b41ffc976958a0fc3481cbd5ddde01>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Components/ActivityIndicator/ActivityIndicator.js
 */

import type { HostComponent } from "../../../src/private/types/HostComponent";
import type { ViewProps } from "../View/ViewPropTypes";
import { type ColorValue } from "../../StyleSheet/StyleSheet";
import * as React from "react";
type IndicatorSize = number | "small" | "large";
type ActivityIndicatorIOSProps = Readonly<{
  /**
    Whether the indicator should hide when not animating.
     @platform ios
  */
  hidesWhenStopped?: boolean | undefined;
}>;
export type ActivityIndicatorProps = Readonly<Omit<ViewProps, keyof ActivityIndicatorIOSProps | keyof {
  /**
  Whether to show the indicator (`true`) or hide it (`false`).
  */
  animating?: boolean | undefined;
  /**
  The foreground color of the spinner.
  @default {@platform android} `null` (system accent default color)
  @default {@platform ios} '#999999'
  */
  color?: ColorValue | undefined;
  /**
  Size of the indicator.
  @type enum(`'small'`, `'large'`)
  @type {@platform android} number
  */
  size?: IndicatorSize | undefined;
}> & Omit<ActivityIndicatorIOSProps, keyof {
  /**
  Whether to show the indicator (`true`) or hide it (`false`).
  */
  animating?: boolean | undefined;
  /**
  The foreground color of the spinner.
  @default {@platform android} `null` (system accent default color)
  @default {@platform ios} '#999999'
  */
  color?: ColorValue | undefined;
  /**
  Size of the indicator.
  @type enum(`'small'`, `'large'`)
  @type {@platform android} number
  */
  size?: IndicatorSize | undefined;
}> & {
  /**
  Whether to show the indicator (`true`) or hide it (`false`).
  */
  animating?: boolean | undefined;
  /**
  The foreground color of the spinner.
  @default {@platform android} `null` (system accent default color)
  @default {@platform ios} '#999999'
  */
  color?: ColorValue | undefined;
  /**
  Size of the indicator.
  @type enum(`'small'`, `'large'`)
  @type {@platform android} number
  */
  size?: IndicatorSize | undefined;
}>;
declare const ActivityIndicator: (props: Omit<ActivityIndicatorProps, keyof {
  ref?: React.Ref<HostComponent<never>>;
}> & {
  ref?: React.Ref<HostComponent<never>>;
}) => React.ReactNode;
declare const $$ActivityIndicator: typeof ActivityIndicator;
declare type $$ActivityIndicator = typeof $$ActivityIndicator;
export default $$ActivityIndicator;
