"use strict";

const VERSION = require("../../package.json").version;
const crypto = require("crypto");
const getCacheKey = require("metro-cache-key");
function getTransformCacheKey(opts) {
  const { transformerPath, transformerConfig } = opts.transformerConfig;
  const Transformer = require.call(null, transformerPath);
  const transformerKey = Transformer.getCacheKey
    ? Transformer.getCacheKey(transformerConfig)
    : "";
  return crypto
    .createHash("sha1")
    .update(
      [
        "metro-cache",
        VERSION,
        opts.cacheVersion,
        getCacheKey([require.resolve(transformerPath)]),
        transformerKey,
        transformerConfig.globalPrefix,
      ].join("$")
    )
    .digest("hex");
}
module.exports = getTransformCacheKey;
