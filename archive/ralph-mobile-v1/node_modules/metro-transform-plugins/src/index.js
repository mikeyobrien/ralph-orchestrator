"use strict";

module.exports = {
  get addParamsToDefineCall() {
    return require("./addParamsToDefineCall").default;
  },
  get constantFoldingPlugin() {
    return require("./constant-folding-plugin").default;
  },
  get importExportPlugin() {
    return require("./import-export-plugin").default;
  },
  get inlinePlugin() {
    return require("./inline-plugin").default;
  },
  get inlineRequiresPlugin() {
    return require("./inline-requires-plugin").default;
  },
  get normalizePseudoGlobals() {
    return require("./normalizePseudoGlobals").default;
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
