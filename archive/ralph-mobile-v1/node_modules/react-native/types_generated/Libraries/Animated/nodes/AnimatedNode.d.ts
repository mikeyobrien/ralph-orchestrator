/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<963cf69679c74d08c66b749959a3adeb>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Animated/nodes/AnimatedNode.js
 */

export type AnimatedNodeConfig = Readonly<{
  debugID?: string;
}>;
declare class AnimatedNode {
  constructor(config?: null | undefined | Readonly<Omit<AnimatedNodeConfig, keyof {}> & {}>);
  /**
   * Adds an asynchronous listener to the value so you can observe updates from
   * animations.  This is useful because there is no way to
   * synchronously read the value because it might be driven natively.
   *
   * See https://reactnative.dev/docs/animatedvalue#addlistener
   */
  addListener(callback: (value: any) => unknown): string;
  /**
   * Unregister a listener. The `id` param shall match the identifier
   * previously returned by `addListener()`.
   *
   * See https://reactnative.dev/docs/animatedvalue#removelistener
   */
  removeListener(id: string): void;
  /**
   * Remove all registered listeners.
   *
   * See https://reactnative.dev/docs/animatedvalue#removealllisteners
   */
  removeAllListeners(): void;
  hasListeners(): boolean;
  /**
   * NOTE: This is intended to prevent `JSON.stringify` from throwing "cyclic
   * structure" errors in React DevTools. Avoid depending on this!
   */
  toJSON(): unknown;
}
export default AnimatedNode;
