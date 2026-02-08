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

/**
 * Originally vendored from https://github.com/amasad/sane/blob/64ff3a870c42e84f744086884bf55a4f9c22d376/src/node_watcher.js
 */

import type { ChangeEventMetadata, WatcherBackendChangeEvent } from "../flow-types";
import type { FSWatcher } from "node:fs";
import { AbstractWatcher } from "./AbstractWatcher";
declare class FallbackWatcher extends AbstractWatcher {
  readonly _changeTimers: Map<string, NodeJS.Timeout>;
  readonly _dirRegistry: {
    [directory: string]: {
      [file: string]: true;
    };
  };
  readonly watched: {
    [key: string]: FSWatcher;
  };
  startWatching(): Promise<void>;
  _register(filepath: string, type: ChangeEventMetadata["type"]): boolean;
  _unregister(filepath: string): void;
  _unregisterDir(dirpath: string): void;
  _registered(fullpath: string): boolean;
  _checkedEmitError: (error: Error) => void;
  _watchdir: ($$PARAM_0$$: string) => boolean;
  _stopWatching(dir: string): Promise<void>;
  stopWatching(): Promise<void>;
  _detectChangedFile(dir: string, event: string, callback: (file: string) => void): void;
  _normalizeChange(dir: string, event: string, file: string): void;
  _processChange(dir: string, event: string, file: string): Promise<void>;
  _emitEvent(change: Omit<WatcherBackendChangeEvent, "root">): void;
  getPauseReason(): null | undefined | string;
}
export default FallbackWatcher;