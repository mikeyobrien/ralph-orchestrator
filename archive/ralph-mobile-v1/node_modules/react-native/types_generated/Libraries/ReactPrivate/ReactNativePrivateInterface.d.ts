/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<54658528d2b0bbd1be2b2c10b4e41d2c>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/ReactPrivate/ReactNativePrivateInterface.js.flow
 */

import type { createPublicTextInstance as $$IMPORT_TYPEOF_1$$ } from "../ReactNative/ReactFabricPublicInstance/ReactFabricPublicInstance";
type createPublicTextInstanceT = typeof $$IMPORT_TYPEOF_1$$;
export type { HostInstance as PublicInstance, NativeMethods as LegacyPublicInstance, MeasureOnSuccessCallback, MeasureInWindowOnSuccessCallback, MeasureLayoutOnSuccessCallback } from "../../src/private/types/HostInstance";
export type { PublicRootInstance } from "../ReactNative/ReactFabricPublicInstance/ReactFabricPublicInstance";
export type PublicTextInstance = ReturnType<createPublicTextInstanceT>;
export { default as BatchedBridge } from "../BatchedBridge/BatchedBridge";
export { default as ExceptionsManager } from "../Core/ExceptionsManager";
export { default as Platform } from "../Utilities/Platform";
export { default as RCTEventEmitter } from "../EventEmitter/RCTEventEmitter";
export * as ReactNativeViewConfigRegistry from "../Renderer/shims/ReactNativeViewConfigRegistry";
export { default as TextInputState } from "../Components/TextInput/TextInputState";
export { default as UIManager } from "../ReactNative/UIManager";
export { default as deepDiffer } from "../Utilities/differ/deepDiffer";
export { default as deepFreezeAndThrowOnMutationInDev } from "../Utilities/deepFreezeAndThrowOnMutationInDev";
export { default as flattenStyle } from "../StyleSheet/flattenStyle";
export { default as ReactFiberErrorDialog } from "../Core/ReactFiberErrorDialog";
export { default as legacySendAccessibilityEvent } from "../Components/AccessibilityInfo/legacySendAccessibilityEvent";
export { default as RawEventEmitter } from "../Core/RawEventEmitter";
export { default as CustomEvent } from "../../src/private/webapis/dom/events/CustomEvent";
export { create as createAttributePayload, diff as diffAttributePayloads } from "../ReactNative/ReactFabricPublicInstance/ReactNativeAttributePayload";
export { createPublicRootInstance, createPublicInstance, createPublicTextInstance, getNativeTagFromPublicInstance, getNodeFromPublicInstance, getInternalInstanceHandleFromPublicInstance } from "../ReactNative/ReactFabricPublicInstance/ReactFabricPublicInstance";
