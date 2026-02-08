// See: https://github.com/facebook/metro/blob/v0.83.2/packages/metro-transform-plugins/src/index.js

// NOTE(cedric): this is quite a complicated CJS export, this can't be automatically typed

export type { Options as ImportExportPluginOptions } from './import-export-plugin';
export type { Options as InlinePluginOptions } from './inline-plugin';
export type { PluginOptions as InlineRequiresPluginOptions } from './inline-requires-plugin';

export { default as addParamsToDefineCall } from './addParamsToDefineCall';
export { default as constantFoldingPlugin } from './constant-folding-plugin';
export { default as importExportPlugin } from './import-export-plugin';
export { default as inlinePlugin } from './inline-plugin';
export { default as inlineRequiresPlugin } from './inline-requires-plugin';
export { default as normalizePseudoGlobals } from './normalizePseudoGlobals';

export function getTransformPluginCacheKeyFiles(): string[];
