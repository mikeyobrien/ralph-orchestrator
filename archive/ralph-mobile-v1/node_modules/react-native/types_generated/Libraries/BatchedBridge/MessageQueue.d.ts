/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<80a72068d378f1ecf93f3067933266e9>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/BatchedBridge/MessageQueue.js
 */

export type SpyData = {
  type: number;
  module: string | undefined;
  method: string | number;
  args: unknown[];
};
declare class MessageQueue {
  constructor();
  /**
   * Public APIs
   */

  static spy(spyOrToggle: boolean | ((data: SpyData) => void)): void;
  callFunctionReturnFlushedQueue(module: string, method: string, args: unknown[]): null | [Array<number>, Array<number>, Array<unknown>, number];
  invokeCallbackAndReturnFlushedQueue(cbID: number, args: unknown[]): null | [Array<number>, Array<number>, Array<unknown>, number];
  flushedQueue(): null | [Array<number>, Array<number>, Array<unknown>, number];
  getEventLoopRunningTime(): number;
  registerCallableModule(name: string, module: {}): void;
  registerLazyCallableModule(name: string, factory: ($$PARAM_0$$: void) => {}): void;
  getCallableModule(name: string): {} | null;
  callNativeSyncHook(moduleID: number, methodID: number, params: unknown[], onFail: null | undefined | ((...$$REST$$: unknown[]) => void), onSucc: null | undefined | ((...$$REST$$: unknown[]) => void)): unknown;
  processCallbacks(moduleID: number, methodID: number, params: unknown[], onFail: null | undefined | ((...$$REST$$: unknown[]) => void), onSucc: null | undefined | ((...$$REST$$: unknown[]) => void)): void;
  enqueueNativeCall(moduleID: number, methodID: number, params: unknown[], onFail: null | undefined | ((...$$REST$$: unknown[]) => void), onSucc: null | undefined | ((...$$REST$$: unknown[]) => void)): void;
  createDebugLookup(moduleID: number, name: string, methods: null | undefined | ReadonlyArray<string>): void;
  setReactNativeMicrotasksCallback(fn: () => void): void;
}
export default MessageQueue;
