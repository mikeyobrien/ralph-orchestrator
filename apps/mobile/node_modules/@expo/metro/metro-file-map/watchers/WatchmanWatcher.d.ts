/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 *
 * @format
 * @oncall react_native
 */

import type { WatcherOptions } from "./common";
import type { Client, WatchmanFileChange, WatchmanSubscriptionEvent } from "fb-watchman";
import { AbstractWatcher } from "./AbstractWatcher";
/**
 * Watches `dir`.
 */
declare class WatchmanWatcher extends AbstractWatcher {
  client: Client;
  readonly subscriptionName: string;
  watchProjectInfo: null | undefined | {
    readonly relativePath: string;
    readonly root: string;
  };
  readonly watchmanDeferStates: ReadonlyArray<string>;
  constructor(dir: string, $$PARAM_1$$: WatcherOptions);
  startWatching(): Promise<void>;
  _init(onReady: () => void, onError: (error: Error) => void): void;
  _handleChangeEvent(resp: WatchmanSubscriptionEvent): void;
  _handleFileChange(changeDescriptor: WatchmanFileChange, rawClock: WatchmanSubscriptionEvent["clock"]): void;
  stopWatching(): Promise<void>;
  getPauseReason(): null | undefined | string;
}
export default WatchmanWatcher;