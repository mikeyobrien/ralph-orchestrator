import type { CustomTransformOptions, TransformProfile } from "../metro-babel-transformer";
import type { BasicSourceMap, FBSourceFunctionMap, MetroSourceMapSegmentTuple } from "../metro-source-map";
import type { TransformResultDependency } from "../metro/DeltaBundler";
import type { AllowOptionalDependencies } from "../metro/DeltaBundler/types";
import type { DynamicRequiresBehavior } from "../metro/ModuleGraph/worker/collectDependencies";
export interface MinifierConfig {
  readonly [$$Key$$: string]: any;
}
export interface MinifierOptions {
  code: string;
  map?: null | BasicSourceMap;
  filename: string;
  reserved: ReadonlyArray<string>;
  config: MinifierConfig;
}
export interface MinifierResult {
  code: string;
  map?: BasicSourceMap;
}
export type Minifier = ($$PARAM_0$$: MinifierOptions) => MinifierResult | Promise<MinifierResult>;
export declare const enum _Type {
  script = "script",
  module = "module",
  asset = "asset",
}
export type Type = `${_Type}`;
export interface JsTransformerConfig {
  readonly assetPlugins: ReadonlyArray<string>;
  readonly assetRegistryPath: string;
  readonly asyncRequireModulePath: string;
  readonly babelTransformerPath: string;
  readonly dynamicDepsInPackages: DynamicRequiresBehavior;
  readonly enableBabelRCLookup: boolean;
  readonly enableBabelRuntime?: boolean | string;
  readonly globalPrefix: string;
  readonly hermesParser: boolean;
  readonly minifierConfig: MinifierConfig;
  readonly minifierPath: string;
  readonly optimizationSizeLimit: number;
  readonly publicPath: string;
  readonly allowOptionalDependencies: AllowOptionalDependencies;
  readonly unstable_dependencyMapReservedName?: null | string;
  readonly unstable_disableModuleWrapping: boolean;
  readonly unstable_disableNormalizePseudoGlobals: boolean;
  readonly unstable_compactOutput: boolean;
  readonly unstable_allowRequireContext: boolean;
  readonly unstable_memoizeInlineRequires?: boolean;
  readonly unstable_nonMemoizedInlineRequires?: ReadonlyArray<string>;
  readonly unstable_renameRequire?: boolean;
}
export type { CustomTransformOptions } from "../metro-babel-transformer";
export interface JsTransformOptions {
  readonly customTransformOptions?: CustomTransformOptions;
  readonly dev: boolean;
  readonly experimentalImportSupport?: boolean;
  readonly inlinePlatform: boolean;
  readonly inlineRequires: boolean;
  readonly minify: boolean;
  readonly nonInlinedRequires?: ReadonlyArray<string>;
  readonly platform?: null | string;
  readonly type: Type;
  readonly unstable_memoizeInlineRequires?: boolean;
  readonly unstable_nonMemoizedInlineRequires?: ReadonlyArray<string>;
  readonly unstable_staticHermesOptimizedRequire?: boolean;
  readonly unstable_transformProfile: TransformProfile;
}
export declare const enum _JSFileType {
  jsScript = "js/script",
  jsModule = "js/module",
  jsModuleAsset = "js/module/asset",
}
export type JSFileType = `${_JSFileType}`;
export interface _JsOutput_data {
  readonly code: string;
  readonly lineCount: number;
  readonly map: Array<MetroSourceMapSegmentTuple>;
  readonly functionMap?: null | FBSourceFunctionMap;
}
export interface JsOutput {
  readonly data: _JsOutput_data;
  readonly type: JSFileType;
}
export interface TransformResponse {
  readonly dependencies: ReadonlyArray<TransformResultDependency>;
  readonly output: ReadonlyArray<JsOutput>;
}
export declare const transform: (config: JsTransformerConfig, projectRoot: string, filename: string, data: Buffer, options: JsTransformOptions) => Promise<TransformResponse>;
export declare const getCacheKey: (config: JsTransformerConfig) => string;