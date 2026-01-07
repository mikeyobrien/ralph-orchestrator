"use strict";

module.exports = {
  get addParamsToDefineCall() {
    return require("./addParamsToDefineCall");
  },
  get constantFoldingPlugin() {
    return require("./constant-folding-plugin");
  },
  get importExportPlugin() {
    return require("./import-export-plugin");
  },
  get inlinePlugin() {
    return require("./inline-plugin");
  },
  get inlineRequiresPlugin() {
    return require("./inline-requires-plugin");
  },
  get normalizePseudoGlobals() {
    return require("./normalizePseudoGlobals");
  },
  getTransformPluginCacheKeyFiles: () => [
    require.resolve(__filename),
    require.resolve("./constant-folding-plugin"),
    require.resolve("./import-export-plugin"),
    require.resolve("./inline-plugin"),
    require.resolve("./inline-requires-plugin"),
    require.resolve("./normalizePseudoGlobals"),
  ],
};
