/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<edfadc37a06ca0f0f17e5080293af01a>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/src/private/webapis/dom/events/EventTarget.js
 */

import Event from "./Event";
export type EventCallback = (event: Event) => void;
export type EventHandler = {
  handleEvent(event: Event): void;
};
export type EventListener = EventCallback | EventHandler;
export type EventListenerOptions = Readonly<{
  capture?: boolean;
}>;
export type AddEventListenerOptions = Readonly<Omit<EventListenerOptions, keyof {
  passive?: boolean;
  once?: boolean;
  signal?: AbortSignal;
}> & {
  passive?: boolean;
  once?: boolean;
  signal?: AbortSignal;
}>;
declare class EventTarget {
  addEventListener(type: string, callback: EventListener | null, optionsOrUseCapture?: AddEventListenerOptions | boolean): void;
  removeEventListener(type: string, callback: EventListener, optionsOrUseCapture?: EventListenerOptions | boolean): void;
  dispatchEvent(event: Event): boolean;
  /**
   * This a "protected" method to be overridden by a subclass to allow event
   * propagation.
   *
   * Should implement the "get the parent" algorithm
   * (see https://dom.spec.whatwg.org/#get-the-parent).
   */
  EVENT_TARGET_GET_THE_PARENT_KEY(): EventTarget | null;
  /**
   * This is "protected" method to dispatch trusted events.
   */
  INTERNAL_DISPATCH_METHOD_KEY(event: Event): void;
}
export default EventTarget;
