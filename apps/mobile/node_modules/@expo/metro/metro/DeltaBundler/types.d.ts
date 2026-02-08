import type * as _babel_types from "@babel/types";
/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @flow strict-local
 * @format
 * @oncall react_native
 */

import type { RequireContext } from "../lib/contextModule";
import type { RequireContextParams } from "../ModuleGraph/worker/collectDependencies";
import type { Graph } from "./Graph";
import type { JsTransformOptions } from "../../metro-transform-worker";
import CountingSet from "../lib/CountingSet";
export interface MixedOutput {
  readonly data: any;
  readonly type: string;
}
export declare const enum _AsyncDependencyType {
  async = "async",
  maybeSync = "maybeSync",
  prefetch = "prefetch",
  weak = "weak",
}
export type AsyncDependencyType = `${_AsyncDependencyType}`;
export interface _TransformResultDependency_data {
  /**
   * A locally unique key for this dependency within the current module.
   */
  readonly key: string;
  /**
   * If not null, this dependency is due to a dynamic `import()` or `__prefetchImport()` call.
   */
  readonly asyncType?: AsyncDependencyType | null;
  /**
   * True if the dependency is declared with a static "import x from 'y'" or
   * an import() call.
   */
  readonly isESMImport: boolean;
  /**
   * The dependency is enclosed in a try/catch block.
   */
  readonly isOptional?: boolean;
  readonly locs: ReadonlyArray<_babel_types.SourceLocation>;
  /** Context for requiring a collection of modules. */
  readonly contextParams?: RequireContextParams;
}
export interface TransformResultDependency {
  /**
   * The literal name provided to a require or import call. For example 'foo' in
   * case of `require('foo')`.
   */
  readonly name: string;
  /**
   * Extra data returned by the dependency extractor.
   */
  readonly data: _TransformResultDependency_data;
}
export interface ResolvedDependency {
  readonly absolutePath: string;
  readonly data: TransformResultDependency;
}
export type Dependency = ResolvedDependency | {
  readonly data: TransformResultDependency;
};
export interface Module<T = MixedOutput> {
  readonly dependencies: Map<string, Dependency>;
  readonly inverseDependencies: CountingSet<string>;
  readonly output: ReadonlyArray<T>;
  readonly path: string;
  readonly getSource: () => Buffer;
  readonly unstable_transformResultKey?: null | undefined | string;
}
export interface ModuleData<T = MixedOutput> {
  readonly dependencies: ReadonlyMap<string, Dependency>;
  readonly resolvedContexts: ReadonlyMap<string, RequireContext>;
  readonly output: ReadonlyArray<T>;
  readonly getSource: () => Buffer;
  readonly unstable_transformResultKey?: null | undefined | string;
}
export type Dependencies<T = MixedOutput> = Map<string, Module<T>>;
export type ReadOnlyDependencies<T = MixedOutput> = ReadonlyMap<string, Module<T>>;
export type TransformInputOptions = Omit<JsTransformOptions, "inlinePlatform" | "inlineRequires">;
export interface GraphInputOptions {
  readonly entryPoints: ReadonlySet<string>;
  readonly transformOptions: TransformInputOptions;
}
export interface ReadOnlyGraph<T = MixedOutput> {
  readonly entryPoints: ReadonlySet<string>;
  readonly transformOptions: Readonly<TransformInputOptions>;
  readonly dependencies: ReadOnlyDependencies<T>;
}
export type { Graph };
export interface TransformResult<T = MixedOutput> {
  readonly dependencies: ReadonlyArray<TransformResultDependency>;
  readonly output: ReadonlyArray<T>;
  readonly unstable_transformResultKey?: null | undefined | string;
}
export interface TransformResultWithSource<T = MixedOutput> extends TransformResult<T> {
  readonly getSource: () => Buffer;
}
export type TransformFn<T = MixedOutput> = ($$PARAM_0$$: string, $$PARAM_1$$: null | undefined | RequireContext) => Promise<TransformResultWithSource<T>>;
export type ResolveFn = (from: string, dependency: TransformResultDependency) => BundlerResolution;
export interface AllowOptionalDependenciesWithOptions {
  readonly exclude: Array<string>;
}
export type AllowOptionalDependencies = boolean | AllowOptionalDependenciesWithOptions;
export interface BundlerResolution {
  readonly type: "sourceFile";
  readonly filePath: string;
}
export interface Options<T = MixedOutput> {
  readonly resolve: ResolveFn;
  readonly transform: TransformFn<T>;
  readonly transformOptions: TransformInputOptions;
  readonly onProgress?: null | ((numProcessed: number, total: number) => any);
  readonly lazy: boolean;
  readonly unstable_allowRequireContext: boolean;
  readonly unstable_enablePackageExports: boolean;
  readonly shallow: boolean;
}
export interface DeltaResult<T = MixedOutput> {
  readonly added: Map<string, Module<T>>;
  readonly modified: Map<string, Module<T>>;
  readonly deleted: Set<string>;
  readonly reset: boolean;
}
export interface SerializerOptions<T = MixedOutput> {
  readonly asyncRequireModulePath: string;
  readonly createModuleId: ($$PARAM_0$$: string) => number;
  readonly dev: boolean;
  readonly getRunModuleStatement: (moduleId: number | string, globalPrefix: string) => string;
  readonly globalPrefix: string;
  readonly includeAsyncPaths: boolean;
  readonly inlineSourceMap?: null | boolean;
  readonly modulesOnly: boolean;
  readonly processModuleFilter: (module: Module<T>) => boolean;
  readonly projectRoot: string;
  readonly runBeforeMainModule: ReadonlyArray<string>;
  readonly runModule: boolean;
  readonly serverRoot: string;
  readonly shouldAddToIgnoreList: ($$PARAM_0$$: Module<T>) => boolean;
  readonly sourceMapUrl?: null | string;
  readonly sourceUrl?: null | string;
  readonly getSourceUrl?: null | (($$PARAM_0$$: Module<T>) => string);
}