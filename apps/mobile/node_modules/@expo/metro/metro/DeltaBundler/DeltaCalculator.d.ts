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

import type { DeltaResult, Options } from "./types";
import { Graph } from "./Graph";
import EventEmitter from "node:events";
/**
 * This class is in charge of calculating the delta of changed modules that
 * happen between calls. To do so, it subscribes to file changes, so it can
 * traverse the files that have been changed between calls and avoid having to
 * traverse the whole dependency tree for trivial small changes.
 */
declare class DeltaCalculator<T> extends EventEmitter {
  _changeEventSource: EventEmitter;
  _options: Options<T>;
  _currentBuildPromise: null | undefined | Promise<DeltaResult<T>>;
  _deletedFiles: Set<string>;
  _modifiedFiles: Set<string>;
  _addedFiles: Set<string>;
  _requiresReset: any;
  _graph: Graph<T>;
  constructor(entryPoints: ReadonlySet<string>, changeEventSource: EventEmitter, options: Options<T>);
  end(): void;
  getDelta($$PARAM_0$$: {
    reset: boolean;
    shallow: boolean;
  }): Promise<DeltaResult<T>>;
  getGraph(): Graph<T>;
  _handleMultipleFileChanges: any;
  _handleFileChange: any;
  _getChangedDependencies(modifiedFiles: Set<string>, deletedFiles: Set<string>, addedFiles: Set<string>): Promise<DeltaResult<T>>;
}
export default DeltaCalculator;