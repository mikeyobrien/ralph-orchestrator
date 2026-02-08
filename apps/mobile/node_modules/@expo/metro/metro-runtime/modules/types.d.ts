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

export type ModuleMap = ReadonlyArray<[number, string]>;
export interface Bundle {
  readonly modules: ModuleMap;
  readonly post: string;
  readonly pre: string;
}
export interface DeltaBundle {
  readonly added: ModuleMap;
  readonly modified: ModuleMap;
  readonly deleted: ReadonlyArray<number>;
}
export type BundleVariant = ({
  readonly base: true;
  readonly revisionId: string;
} & Bundle) | ({
  readonly base: false;
  readonly revisionId: string;
} & DeltaBundle);
export interface BundleMetadata {
  readonly pre: number;
  readonly post: number;
  readonly modules: ReadonlyArray<[number, number]>;
}
export interface FormattedError {
  readonly type: string;
  readonly message: string;
  readonly errors: Array<{
    description: string;
  }>;
}
export interface HmrModule {
  readonly module: [number, string];
  readonly sourceMappingURL: string;
  readonly sourceURL: string;
}
export interface HmrUpdate {
  readonly added: ReadonlyArray<HmrModule>;
  readonly deleted: ReadonlyArray<number>;
  readonly isInitialUpdate: boolean;
  readonly modified: ReadonlyArray<HmrModule>;
  readonly revisionId: string;
}
export interface HmrUpdateMessage {
  readonly type: "update";
  readonly body: HmrUpdate;
}
export interface HmrErrorMessage {
  readonly type: "error";
  readonly body: FormattedError;
}
export type HmrClientMessage = {
  readonly type: "register-entrypoints";
  readonly entryPoints: Array<string>;
} | {
  readonly type: "log";
  readonly level?: "trace" | "info" | "warn" | "log" | "group" | "groupCollapsed" | "groupEnd" | "debug";
  readonly data: Array<any>;
} | {
  readonly type: "log-opt-in";
};
export type HmrMessage = {
  readonly type: "bundle-registered";
} | {
  readonly type: "update-start";
  readonly body: {
    readonly isInitialUpdate: boolean;
  };
} | {
  readonly type: "update-done";
} | HmrUpdateMessage | HmrErrorMessage;