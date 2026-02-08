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

import type { HmrUpdate } from "./types";
import EventEmitter from "./vendor/eventemitter3";
export declare const enum _SocketState {
  opening = "opening",
  open = "open",
  closed = "closed",
}
export type SocketState = `${_SocketState}`;
declare class HMRClient extends EventEmitter {
  _isEnabled: boolean;
  _pendingUpdate: HmrUpdate | null;
  _queue: Array<string>;
  _state: SocketState;
  _ws: WebSocket;
  constructor(url: string);
  close(): void;
  send(message: string): void;
  _flushQueue(): void;
  enable(): void;
  disable(): void;
  isEnabled(): boolean;
  hasPendingUpdates(): boolean;
}
export default HMRClient;