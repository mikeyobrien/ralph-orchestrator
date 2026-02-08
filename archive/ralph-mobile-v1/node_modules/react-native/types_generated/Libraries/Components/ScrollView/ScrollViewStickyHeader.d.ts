/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<23abffa92f686a9eb5df0367ebc96319>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Components/ScrollView/ScrollViewStickyHeader.js
 */

import type { LayoutChangeEvent } from "../../Types/CoreEventTypes";
import Animated from "../../Animated/Animated";
import * as React from "react";
export type ScrollViewStickyHeaderProps = Readonly<{
  children?: React.ReactNode;
  nextHeaderLayoutY: number | undefined;
  onLayout: (event: LayoutChangeEvent) => void;
  scrollAnimatedValue: Animated.Value;
  inverted: boolean | undefined;
  scrollViewHeight: number | undefined;
  nativeID?: string | undefined;
  hiddenOnScroll?: boolean | undefined;
}>;
interface Instance extends React.ElementRef<typeof Animated.View> {
  readonly setNextHeaderY: ($$PARAM_0$$: number) => void;
}
declare const ScrollViewStickyHeader: (props: Omit<ScrollViewStickyHeaderProps, keyof {
  ref: React.Ref<Instance>;
}> & {
  ref: React.Ref<Instance>;
}) => React.ReactNode;
declare const $$ScrollViewStickyHeader: typeof ScrollViewStickyHeader;
declare type $$ScrollViewStickyHeader = typeof $$ScrollViewStickyHeader;
export default $$ScrollViewStickyHeader;
