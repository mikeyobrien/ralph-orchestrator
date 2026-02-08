"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
Object.defineProperty(exports, "getDefaultConfig", {
  enumerable: true,
  get: function () {
    return _defaults.default;
  },
});
Object.defineProperty(exports, "loadConfig", {
  enumerable: true,
  get: function () {
    return _loadConfig.loadConfig;
  },
});
Object.defineProperty(exports, "mergeConfig", {
  enumerable: true,
  get: function () {
    return _loadConfig.mergeConfig;
  },
});
Object.defineProperty(exports, "resolveConfig", {
  enumerable: true,
  get: function () {
    return _loadConfig.resolveConfig;
  },
});
var _defaults = _interopRequireDefault(require("./defaults"));
var _loadConfig = require("./loadConfig");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
var _default = (exports.default = {
  getDefaultConfig: _defaults.default,
  loadConfig: _loadConfig.loadConfig,
  mergeConfig: _loadConfig.mergeConfig,
  resolveConfig: _loadConfig.resolveConfig,
});
