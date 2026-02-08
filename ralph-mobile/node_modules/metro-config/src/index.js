"use strict";

try {
  require("metro-babel-register").unstable_registerForMetroMonorepo();
} catch {}
const getDefaultConfig = require("./defaults");
const { loadConfig, mergeConfig, resolveConfig } = require("./loadConfig");
module.exports = {
  loadConfig,
  resolveConfig,
  mergeConfig,
  getDefaultConfig,
};
