import type { NodePath } from "@babel/traverse";
import type { CallExpression, Identifier, StringLiteral, SourceLocation, File } from "@babel/types";
import type { AllowOptionalDependencies, AsyncDependencyType } from "../../DeltaBundler/types";
export interface Dependency {
  readonly data: DependencyData;
  readonly name: string;
}
export declare const enum _ContextMode {
  sync = "sync",
  eager = "eager",
  lazy = "lazy",
  lazyOnce = "lazy-once",
}
export type ContextMode = `${_ContextMode}`;
export interface ContextFilter {
  readonly pattern: string;
  readonly flags: string;
}
export interface RequireContextParams {
  readonly recursive: boolean;
  readonly filter: Readonly<ContextFilter>;
  readonly mode: ContextMode;
}
export interface DependencyData {
  readonly key: string;
  readonly asyncType?: AsyncDependencyType | null;
  readonly isESMImport: boolean;
  readonly isOptional?: boolean;
  readonly locs: ReadonlyArray<SourceLocation>;
  readonly contextParams?: RequireContextParams;
}
export interface MutableInternalDependency extends DependencyData {
  locs: Array<SourceLocation>;
  index: number;
  name: string;
}
export type InternalDependency = Readonly<MutableInternalDependency>;
export interface State {
  asyncRequireModulePathStringLiteral?: null | StringLiteral;
  dependencyCalls: Set<string>;
  dependencyRegistry: DependencyRegistry;
  dependencyTransformer: DependencyTransformer;
  dynamicRequires: DynamicRequiresBehavior;
  dependencyMapIdentifier?: null | Identifier;
  keepRequireNames: boolean;
  allowOptionalDependencies: AllowOptionalDependencies;
  unstable_allowRequireContext: boolean;
  unstable_isESMImportAtSource?: null | (($$PARAM_0$$: SourceLocation) => boolean);
}
export interface Options {
  readonly asyncRequireModulePath: string;
  readonly dependencyMapName?: null | string;
  readonly dynamicRequires: DynamicRequiresBehavior;
  readonly inlineableCalls: ReadonlyArray<string>;
  readonly keepRequireNames: boolean;
  readonly allowOptionalDependencies: AllowOptionalDependencies;
  readonly dependencyTransformer?: DependencyTransformer;
  readonly unstable_allowRequireContext: boolean;
  readonly unstable_isESMImportAtSource?: null | undefined | (($$PARAM_0$$: SourceLocation) => boolean);
}
export interface CollectedDependencies {
  readonly ast: File;
  readonly dependencyMapName: string;
  readonly dependencies: ReadonlyArray<Dependency>;
}
export interface DependencyTransformer {
  transformSyncRequire(path: NodePath<CallExpression>, dependency: InternalDependency, state: State): void;
  transformImportCall(path: NodePath, dependency: InternalDependency, state: State): void;
  transformImportMaybeSyncCall(path: NodePath, dependency: InternalDependency, state: State): void;
  transformPrefetch(path: NodePath, dependency: InternalDependency, state: State): void;
  transformIllegalDynamicRequire(path: NodePath, state: State): void;
}
export declare const enum _DynamicRequiresBehavior {
  throwAtRuntime = "throwAtRuntime",
  reject = "reject",
}
export type DynamicRequiresBehavior = `${_DynamicRequiresBehavior}`;
declare function collectDependencies(ast: File, options: Options): CollectedDependencies;
export default collectDependencies;
export interface ImportQualifier {
  readonly name: string;
  readonly asyncType?: AsyncDependencyType | null;
  readonly isESMImport: boolean;
  readonly optional: boolean;
  readonly contextParams?: RequireContextParams;
}
declare class DependencyRegistry {
  _dependencies: Map<string, InternalDependency>;
  registerDependency(qualifier: ImportQualifier): InternalDependency;
  getDependencies(): Array<InternalDependency>;
}