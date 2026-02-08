"use strict";

const { mergeConfig } = require("../loadConfig");
const secondConfig = (previous) => ({
  resolver: {
    sourceExts: ["before", ...previous.resolver.sourceExts],
  },
});
const thirdConfig = (previous) => ({
  resolver: {
    sourceExts: [...previous.resolver.sourceExts, "after"],
  },
});
module.exports = (metroDefaults) =>
  mergeConfig(metroDefaults, secondConfig, thirdConfig);
