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
 * Portions of this code are based on the Synchronous Cycle Collection
 * algorithm described in:
 *
 * David F. Bacon and V. T. Rajan. 2001. Concurrent Cycle Collection in
 * Reference Counted Systems. In Proceedings of the 15th European Conference on
 * Object-Oriented Programming (ECOOP '01). Springer-Verlag, Berlin,
 * Heidelberg, 207–235.
 *
 * Notable differences from the algorithm in the paper:
 * 1. Our implementation uses the inverseDependencies set (which we already
 *    have to maintain) instead of a separate refcount variable. A module's
 *    reference count is equal to the size of its inverseDependencies set, plus
 *    1 if it's an entry point of the graph.
 * 2. We keep the "root buffer" (possibleCycleRoots) free of duplicates by
 *    making it a Set, instead of storing a "buffered" flag on each node.
 * 3. On top of tracking edges between nodes, we also count references between
 *    nodes and entries in the importBundleNodes set.
 */

import type { RequireContext } from "../lib/contextModule";
import type { Dependencies, Dependency, GraphInputOptions, MixedOutput, Module, ModuleData, Options, ResolvedDependency, TransformInputOptions } from "./types";
import CountingSet from "../lib/CountingSet";
export interface Result<T> {
  added: Map<string, Module<T>>;
  modified: Map<string, Module<T>>;
  deleted: Set<string>;
}
/**
 * Internal data structure that the traversal logic uses to know which of the
 * files have been modified. This allows to return the added modules before the
 * modified ones (which is useful for things like Hot Module Reloading).
 **/
/**
 * Internal data structure that the traversal logic uses to know which of the
 * files have been modified. This allows to return the added modules before the
 * modified ones (which is useful for things like Hot Module Reloading).
 **/
export interface Delta<T> {
  readonly added: Set<string>;
  readonly touched: Set<string>;
  readonly deleted: Set<string>;
  readonly updatedModuleData: ReadonlyMap<string, ModuleData<T>>;
  readonly baseModuleData: Map<string, ModuleData<T>>;
  readonly errors: ReadonlyMap<string, Error>;
}
export interface InternalOptions<T> {
  readonly lazy: boolean;
  readonly onDependencyAdd: () => any;
  readonly onDependencyAdded: () => any;
  readonly resolve: Options<T>["resolve"];
  readonly transform: Options<T>["transform"];
  readonly shallow: boolean;
}
export declare class Graph<T = MixedOutput> {
  readonly entryPoints: ReadonlySet<string>;
  readonly transformOptions: TransformInputOptions;
  readonly dependencies: Dependencies<T>;
  constructor(options: GraphInputOptions);
  traverseDependencies(paths: ReadonlyArray<string>, options: Options<T>): Promise<Result<T>>;
  initialTraverseDependencies(options: Options<T>): Promise<Result<T>>;
  _buildDelta(pathsToVisit: ReadonlySet<string>, options: InternalOptions<T>, moduleFilter?: (path: string) => boolean): Promise<Delta<T>>;
  _recursivelyCommitModule(path: string, delta: Delta<T>, options: InternalOptions<T>, commitOptions: {
    readonly onlyRemove: boolean;
  }): Module<T>;
  _addDependency(parentModule: Module<T>, key: string, dependency: Dependency, requireContext: null | undefined | RequireContext, delta: Delta<T>, options: InternalOptions<T>): void;
  _removeDependency(parentModule: Module<T>, key: string, dependency: Dependency, delta: Delta<T>, options: InternalOptions<T>): void;
  markModifiedContextModules(filePath: string, modifiedPaths: Set<string> | CountingSet<string>): void;
  getModifiedModulesForDeletedPath(filePath: string): Iterable<string>;
  reorderGraph(options: {
    shallow: boolean;
  }): void;
  _reorderDependencies(module: Module<T>, orderedDependencies: Map<string, Module<T>>, options: {
    shallow: boolean;
  }): void;
  _incrementImportBundleReference(dependency: ResolvedDependency, parentModule: Module<T>): void;
  _decrementImportBundleReference(dependency: ResolvedDependency, parentModule: Module<T>): void;
  _markModuleInUse(module: Module<T>): void;
  _children(module: Module<T>, options: InternalOptions<T>): Iterator<Module<T>>;
  _moduleSnapshot(module: Module<T>): ModuleData<T>;
  _releaseModule(module: Module<T>, delta: Delta<T>, options: InternalOptions<T>): void;
  _freeModule(module: Module<T>, delta: Delta<T>): void;
  _markAsPossibleCycleRoot(module: Module<T>): void;
  _collectCycles(delta: Delta<T>, options: InternalOptions<T>): void;
  _markGray(module: Module<T>, options: InternalOptions<T>): void;
  _scan(module: Module<T>, options: InternalOptions<T>): void;
  _scanBlack(module: Module<T>, options: InternalOptions<T>): void;
  _collectWhite(module: Module<T>, delta: Delta<T>): void;
}