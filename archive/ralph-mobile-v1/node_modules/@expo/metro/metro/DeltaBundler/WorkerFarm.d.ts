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

import type { TransformResult } from "../DeltaBundler";
import type { TransformerConfig, TransformOptions, Worker } from "./Worker";
import type { ConfigT } from "../../metro-config";
import type { Readable } from "node:stream";
export interface WorkerInterface extends Worker {
  getStdout(): Readable;
  getStderr(): Readable;
  end(): void;
}
export interface TransformerResult {
  readonly result: TransformResult;
  readonly sha1: string;
}
declare class WorkerFarm {
  _config: ConfigT;
  _transformerConfig: TransformerConfig;
  _worker: WorkerInterface | Worker;
  constructor(config: ConfigT, transformerConfig: TransformerConfig);
  kill(): Promise<void>;
  transform(filename: string, options: TransformOptions, fileBuffer?: Buffer): Promise<TransformerResult>;
  _makeFarm(absoluteWorkerPath: string, exposedMethods: ReadonlyArray<string>, numWorkers: number): any;
  _computeWorkerKey(method: string, filename: string): null | undefined | string;
  _formatGenericError(err: any, filename: string): TransformError;
  _formatBabelError(err: any, filename: string): TransformError;
}
export default WorkerFarm;
declare class TransformError extends SyntaxError {
  type: string;
  constructor(message: string);
}