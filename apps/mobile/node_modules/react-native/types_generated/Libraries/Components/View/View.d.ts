/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<aa1f84d5d6895f42982529f752c54b9e>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Components/View/View.js
 */

import type { ViewProps } from "./ViewPropTypes";
import ViewNativeComponent from "./ViewNativeComponent";
import * as React from "react";
/**
 * The most fundamental component for building a UI, View is a container that
 * supports layout with flexbox, style, some touch handling, and accessibility
 * controls.
 *
 * @see https://reactnative.dev/docs/view
 */
declare function View(props: Omit<ViewProps, keyof {
  ref?: React.Ref<React.ComponentRef<typeof ViewNativeComponent>>;
}> & {
  ref?: React.Ref<React.ComponentRef<typeof ViewNativeComponent>>;
}): React.ReactNode;
declare const $$View: typeof View;
declare type $$View = typeof $$View;
export default $$View;
