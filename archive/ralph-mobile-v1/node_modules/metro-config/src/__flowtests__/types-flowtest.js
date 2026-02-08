"use strict";

var _loadConfig = require("../loadConfig");
config;
({});
({
  resolver: {},
  transformer: {},
  serializer: {},
  server: {},
  symbolicator: {},
});
isMutableArray(config.cacheStores);
if (
  inputConfig.cacheStores != null &&
  typeof inputConfig.cacheStores !== "function"
) {
  isMutableArray(inputConfig.cacheStores);
}
config.resolver.unstable_conditionsByPlatform["foo"];
config.transformer.assetPlugins[0];
(0, _loadConfig.mergeConfig)(config, {});
(0, _loadConfig.mergeConfig)(inputConfig, {});
(0, _loadConfig.mergeConfig)(
  config,
  () => ({}),
  {},
  () => ({}),
);
(0, _loadConfig.mergeConfig)(
  config,
  () => ({}),
  {},
  async () => ({}),
).catch(() => {});
