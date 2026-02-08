/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<2599e35b506b6441c1270839d2b99e09>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Image/TextInlineImageNativeComponent.js
 */

import type { HostComponent } from "../../src/private/types/HostComponent";
import type { ViewProps } from "../Components/View/ViewPropTypes";
import type { PartialViewConfig } from "../Renderer/shims/ReactNativeTypes";
import type { ColorValue } from "../StyleSheet/StyleSheet";
import type { ImageResizeMode } from "./ImageResizeMode";
type RCTTextInlineImageNativeProps = Readonly<Omit<ViewProps, keyof {
  resizeMode?: ImageResizeMode | undefined;
  src?: ReadonlyArray<Readonly<{
    uri?: string | undefined;
  }> | undefined> | undefined;
  tintColor?: ColorValue | undefined;
  headers?: {
    [$$Key$$: string]: string;
  } | undefined;
}> & {
  resizeMode?: ImageResizeMode | undefined;
  src?: ReadonlyArray<Readonly<{
    uri?: string | undefined;
  }> | undefined> | undefined;
  tintColor?: ColorValue | undefined;
  headers?: {
    [$$Key$$: string]: string;
  } | undefined;
}>;
export declare const __INTERNAL_VIEW_CONFIG: PartialViewConfig;
export declare type __INTERNAL_VIEW_CONFIG = typeof __INTERNAL_VIEW_CONFIG;
declare const TextInlineImage: HostComponent<RCTTextInlineImageNativeProps>;
declare const $$TextInlineImageNativeComponent: typeof TextInlineImage;
declare type $$TextInlineImageNativeComponent = typeof $$TextInlineImageNativeComponent;
export default $$TextInlineImageNativeComponent;
