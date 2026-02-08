/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<48a605c48a65a85b29efde935b0914f0>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/Linking/Linking.js
 */

import type { EventSubscription } from "../vendor/emitter/EventEmitter";
import NativeEventEmitter from "../EventEmitter/NativeEventEmitter";
type LinkingEventDefinitions = {
  url: [{
    url: string;
  }];
};
declare class LinkingImpl extends NativeEventEmitter<LinkingEventDefinitions> {
  constructor();
  /**
   * Add a handler to Linking changes by listening to the `url` event type
   * and providing the handler
   *
   * See https://reactnative.dev/docs/linking#addeventlistener
   */
  addEventListener<K extends keyof LinkingEventDefinitions>(eventType: K, listener: (...$$REST$$: LinkingEventDefinitions[K]) => unknown): EventSubscription;
  /**
   * Try to open the given `url` with any of the installed apps.
   *
   * See https://reactnative.dev/docs/linking#openurl
   */
  openURL(url: string): Promise<void>;
  /**
   * Determine whether or not an installed app can handle a given URL.
   *
   * See https://reactnative.dev/docs/linking#canopenurl
   */
  canOpenURL(url: string): Promise<boolean>;
  /**
   * Open app settings.
   *
   * See https://reactnative.dev/docs/linking#opensettings
   */
  openSettings(): Promise<void>;
  /**
   * If the app launch was triggered by an app link,
   * it will give the link url, otherwise it will give `null`
   *
   * See https://reactnative.dev/docs/linking#getinitialurl
   */
  getInitialURL(): Promise<null | undefined | string>;
  sendIntent(action: string, extras?: Array<{
    key: string;
    value: string | number | boolean;
  }>): Promise<void>;
}
declare const Linking: LinkingImpl;
/**
 * `Linking` gives you a general interface to interact with both incoming
 * and outgoing app links.
 *
 * See https://reactnative.dev/docs/linking
 */
declare const $$Linking: typeof Linking;
declare type $$Linking = typeof $$Linking;
export default $$Linking;
