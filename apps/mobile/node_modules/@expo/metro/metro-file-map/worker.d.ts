/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 *
 * @format
 */

import type { WorkerMessage, WorkerMetadata, WorkerSetupArgs } from "./flow-types";
export declare function setup(args: WorkerSetupArgs): void;
export declare function processFile(data: WorkerMessage): WorkerMetadata;
export declare class Worker {
  constructor(args: WorkerSetupArgs);
  processFile(data: WorkerMessage): WorkerMetadata;
}