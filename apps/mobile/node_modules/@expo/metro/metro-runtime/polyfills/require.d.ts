/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 *
 * @format
 * @oncall react_native
 * @polyfill
 */

type ArrayIndexable<T> = {
  readonly [indexer: number]: T;
};
export interface _DependencyMap_paths {
  [id: ModuleID]: string;
}
export interface DependencyMap extends Readonly<ArrayIndexable<ModuleID>> {
  readonly paths?: _DependencyMap_paths;
}
export interface InverseDependencyMap {
  [key: ModuleID]: Array<ModuleID>;
}
type Exports = any;
type FactoryFn = (global: Object, require: RequireFn, metroImportDefault: RequireFn, metroImportAll: RequireFn, moduleObject: {
  exports: {};
}, exports: {}, dependencyMap: null | undefined | DependencyMap) => void;
type ModuleID = number;
export type RequireFn = (id: ModuleID | VerboseModuleNameForDev) => Exports;
export type DefineFn = (factory: FactoryFn, moduleId: number, dependencyMap?: DependencyMap, verboseName?: string, inverseDependencies?: InverseDependencyMap) => void;
type VerboseModuleNameForDev = string;