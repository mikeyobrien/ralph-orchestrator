/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 *
 */

import type { Console, CrawlerOptions, FileData, Path, PerfLogger, WatcherBackend, WatcherBackendChangeEvent, WatchmanClocks } from "./flow-types";
import EventEmitter from "node:events";
export interface CrawlResult {
  changedFiles: FileData;
  clocks?: WatchmanClocks;
  removedFiles: Set<Path>;
}
export interface WatcherOptions {
  abortSignal: AbortSignal;
  computeSha1: boolean;
  console: Console;
  enableSymlinks: boolean;
  extensions: ReadonlyArray<string>;
  forceNodeFilesystemAPI: boolean;
  healthCheckFilePrefix: string;
  ignoreForCrawl: ($$PARAM_0$$: string) => boolean;
  ignorePatternForWatch: RegExp;
  previousState: CrawlerOptions["previousState"];
  perfLogger?: null | PerfLogger;
  roots: ReadonlyArray<string>;
  rootDir: string;
  useWatchman: boolean;
  watch: boolean;
  watchmanDeferStates: ReadonlyArray<string>;
}
export type HealthCheckResult = {
  type: "error";
  timeout: number;
  error: Error;
  watcher?: null | string;
} | {
  type: "success";
  timeout: number;
  timeElapsed: number;
  watcher?: null | string;
} | {
  type: "timeout";
  timeout: number;
  watcher?: null | string;
  pauseReason?: null | string;
};
export declare class Watcher extends EventEmitter {
  _options: WatcherOptions;
  _backends: ReadonlyArray<WatcherBackend>;
  _instanceId: number;
  _nextHealthCheckId: number;
  _pendingHealthChecks: Map<string, () => void>;
  _activeWatcher: null | undefined | string;
  constructor(options: WatcherOptions);
  crawl(): Promise<CrawlResult>;
  watch(onChange: (change: WatcherBackendChangeEvent) => void): Promise<void>;
  _handleHealthCheckObservation(basename: string): void;
  close(): Promise<void>;
  checkHealth(timeout: number): Promise<HealthCheckResult>;
}