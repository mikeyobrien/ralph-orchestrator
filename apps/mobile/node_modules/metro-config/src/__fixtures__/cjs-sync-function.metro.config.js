"use strict";

module.exports = (defaultConfig) => ({
  cacheStores: [],
  reporter: undefined,
  maxWorkers: 2,
  resolver: {
    sourceExts: [...defaultConfig.resolver.sourceExts, "tsx"],
    hasteImplModulePath: "test",
  },
  transformerPath: "",
});
