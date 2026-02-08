"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = getTransformCacheKey;
var _crypto = _interopRequireDefault(require("crypto"));
var _metroCacheKey = require("metro-cache-key");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const VERSION = require("../../package.json").version;
function getTransformCacheKey(opts) {
  const { transformerPath, transformerConfig } = opts.transformerConfig;
  const Transformer = require.call(null, transformerPath);
  const transformerKey = Transformer.getCacheKey
    ? Transformer.getCacheKey(transformerConfig)
    : "";
  return _crypto.default
    .createHash("sha1")
    .update(
      [
        "metro-cache",
        VERSION,
        opts.cacheVersion,
        (0, _metroCacheKey.getCacheKey)([require.resolve(transformerPath)]),
        transformerKey,
        transformerConfig.globalPrefix,
      ].join("$"),
    )
    .digest("hex");
}
