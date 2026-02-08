/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<c84e4fae3550c41655c5967750fee0f0>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/src/private/webapis/dom/nodes/ReactNativeElement.js
 */

import type { ViewConfig } from "../../../../../Libraries/Renderer/shims/ReactNativeTypes";
import type { HostInstance, MeasureInWindowOnSuccessCallback, MeasureLayoutOnSuccessCallback, MeasureOnSuccessCallback, NativeMethods } from "../../../types/HostInstance";
import type { InstanceHandle } from "./internals/NodeInternals";
import type ReactNativeDocument from "./ReactNativeDocument";
import ReadOnlyElement from "./ReadOnlyElement";
declare class ReactNativeElement extends ReadOnlyElement implements NativeMethods {
  constructor(tag: number, viewConfig: ViewConfig, instanceHandle: InstanceHandle, ownerDocument: ReactNativeDocument);
  get offsetHeight(): number;
  get offsetLeft(): number;
  get offsetParent(): ReadOnlyElement | null;
  get offsetTop(): number;
  get offsetWidth(): number;
  /**
   * React Native compatibility methods
   */

  blur(): void;
  focus(): void;
  measure(callback: MeasureOnSuccessCallback): void;
  measureInWindow(callback: MeasureInWindowOnSuccessCallback): void;
  measureLayout(relativeToNativeNode: number | HostInstance, onSuccess: MeasureLayoutOnSuccessCallback, onFail?: () => void): void;
  setNativeProps(nativeProps: {}): void;
}
export default ReactNativeElement;
