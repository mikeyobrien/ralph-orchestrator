/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<756713a532f27fbe10496ce66b7a8625>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Components/Keyboard/Keyboard.js
 */

import type { EventSubscription } from "../../vendor/emitter/EventEmitter";
export type KeyboardEventName = keyof KeyboardEventDefinitions;
export type KeyboardEventEasing = "easeIn" | "easeInEaseOut" | "easeOut" | "linear" | "keyboard";
export type KeyboardMetrics = Readonly<{
  screenX: number;
  screenY: number;
  width: number;
  height: number;
}>;
export type KeyboardEvent = AndroidKeyboardEvent | IOSKeyboardEvent;
type BaseKeyboardEvent = {
  duration: number;
  easing: KeyboardEventEasing;
  endCoordinates: KeyboardMetrics;
};
export type AndroidKeyboardEvent = Readonly<Omit<BaseKeyboardEvent, keyof {
  duration: 0;
  easing: "keyboard";
}> & {
  duration: 0;
  easing: "keyboard";
}>;
export type IOSKeyboardEvent = Readonly<Omit<BaseKeyboardEvent, keyof {
  startCoordinates: KeyboardMetrics;
  isEventFromThisApp: boolean;
}> & {
  startCoordinates: KeyboardMetrics;
  isEventFromThisApp: boolean;
}>;
type KeyboardEventDefinitions = {
  keyboardWillShow: [KeyboardEvent];
  keyboardDidShow: [KeyboardEvent];
  keyboardWillHide: [KeyboardEvent];
  keyboardDidHide: [KeyboardEvent];
  keyboardWillChangeFrame: [KeyboardEvent];
  keyboardDidChangeFrame: [KeyboardEvent];
};
/**
 * `Keyboard` module to control keyboard events.
 *
 * ### Usage
 *
 * The Keyboard module allows you to listen for native events and react to them, as
 * well as make changes to the keyboard, like dismissing it.
 *
 *```
 * import React, { Component } from 'react';
 * import { Keyboard, TextInput } from 'react-native';
 *
 * class Example extends Component {
 *   componentWillMount () {
 *     this.keyboardDidShowListener = Keyboard.addListener('keyboardDidShow', this._keyboardDidShow);
 *     this.keyboardDidHideListener = Keyboard.addListener('keyboardDidHide', this._keyboardDidHide);
 *   }
 *
 *   componentWillUnmount () {
 *     this.keyboardDidShowListener.remove();
 *     this.keyboardDidHideListener.remove();
 *   }
 *
 *   _keyboardDidShow () {
 *     alert('Keyboard Shown');
 *   }
 *
 *   _keyboardDidHide () {
 *     alert('Keyboard Hidden');
 *   }
 *
 *   render() {
 *     return (
 *       <TextInput
 *         onSubmitEditing={Keyboard.dismiss}
 *       />
 *     );
 *   }
 * }
 *```
 */

declare class KeyboardImpl {
  constructor();
  /**
   * The `addListener` function connects a JavaScript function to an identified native
   * keyboard notification event.
   *
   * This function then returns the reference to the listener.
   *
   * @param {string} eventName The `nativeEvent` is the string that identifies the event you're listening for.  This
   *can be any of the following:
   *
   * - `keyboardWillShow`
   * - `keyboardDidShow`
   * - `keyboardWillHide`
   * - `keyboardDidHide`
   * - `keyboardWillChangeFrame`
   * - `keyboardDidChangeFrame`
   *
   * Android versions prior to API 30 rely on observing layout changes when
   * `android:windowSoftInputMode` is set to `adjustResize` or `adjustPan`.
   *
   * `keyboardWillShow` as well as `keyboardWillHide` are not available on Android since there is
   * no native corresponding event.
   *
   * @param {function} callback function to be called when the event fires.
   */
  addListener<K extends keyof KeyboardEventDefinitions>(eventType: K, listener: (...$$REST$$: KeyboardEventDefinitions[K]) => unknown, context?: unknown): EventSubscription;
  /**
   * Removes all listeners for a specific event type.
   *
   * @param {string} eventType The native event string listeners are watching which will be removed.
   */
  removeAllListeners<K extends keyof KeyboardEventDefinitions>(eventType: null | undefined | K): void;
  /**
   * Dismisses the active keyboard and removes focus.
   */
  dismiss(): void;
  /**
   * Whether the keyboard is last known to be visible.
   */
  isVisible(): boolean;
  /**
   * Return the metrics of the soft-keyboard if visible.
   */
  metrics(): null | undefined | KeyboardMetrics;
  /**
   * Useful for syncing TextInput (or other keyboard accessory view) size of
   * position changes with keyboard movements.
   */
  scheduleLayoutAnimation(event: KeyboardEvent): void;
}
declare const Keyboard: KeyboardImpl;
declare const $$Keyboard: typeof Keyboard;
declare type $$Keyboard = typeof $$Keyboard;
export default $$Keyboard;
