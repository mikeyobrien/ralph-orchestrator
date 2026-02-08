/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @generated SignedSource<<7cf4d721fb5a7731bd3ece5458aeb171>>
 *
 * This file was translated from Flow by scripts/build-types/index.js.
 * Original file: packages/react-native/Libraries/LogBox/LogBox.js
 */

import type { IgnorePattern, LogData } from "./Data/LogBoxData";
import type { ExtendedExceptionData } from "./Data/parseLogBoxLog";
export type { LogData, ExtendedExceptionData, IgnorePattern };
interface ILogBox {
  install(): void;
  uninstall(): void;
  isInstalled(): boolean;
  ignoreLogs($$PARAM_0$$: ReadonlyArray<IgnorePattern>): void;
  ignoreAllLogs(value?: boolean): void;
  clearAllLogs(): void;
  addLog(log: LogData): void;
  addConsoleLog(level: "warn" | "error", ...args: Array<unknown>): void;
  addException(error: ExtendedExceptionData): void;
}
declare const $$LogBox: ILogBox;
declare type $$LogBox = typeof $$LogBox;
export default $$LogBox;
