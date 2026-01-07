"use strict";

var _crypto = _interopRequireDefault(require("crypto"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const getTransformCacheKey = require("./getTransformCacheKey");
const WorkerFarm = require("./WorkerFarm");
const assert = require("assert");
const fs = require("fs");
const { Cache, stableHash } = require("metro-cache");
const path = require("path");
class Transformer {
  constructor(config, getSha1FnOrOpts) {
    this._config = config;
    this._config.watchFolders.forEach(verifyRootExists);
    this._cache = new Cache(config.cacheStores);
    this._getSha1 =
      typeof getSha1FnOrOpts === "function"
        ? getSha1FnOrOpts
        : getSha1FnOrOpts.unstable_getOrComputeSha1;
    const {
      getTransformOptions: _getTransformOptions,
      transformVariants: _transformVariants,
      workerPath: _workerPath,
      unstable_workerThreads: _workerThreads,
      ...transformerConfig
    } = this._config.transformer;
    const transformerOptions = {
      transformerPath: this._config.transformerPath,
      transformerConfig,
    };
    this._workerFarm = new WorkerFarm(config, transformerOptions);
    const globalCacheKey = this._cache.isDisabled
      ? ""
      : getTransformCacheKey({
          cacheVersion: this._config.cacheVersion,
          projectRoot: this._config.projectRoot,
          transformerConfig: transformerOptions,
        });
    this._baseHash = stableHash([globalCacheKey]).toString("binary");
  }
  async transformFile(filePath, transformerOptions, fileBuffer) {
    const cache = this._cache;
    const {
      customTransformOptions,
      dev,
      experimentalImportSupport,
      hot,
      inlinePlatform,
      inlineRequires,
      minify,
      nonInlinedRequires,
      platform,
      type,
      unstable_disableES6Transforms,
      unstable_transformProfile,
      unstable_memoizeInlineRequires,
      unstable_nonMemoizedInlineRequires,
      ...extra
    } = transformerOptions;
    for (const key in extra) {
      if (hasOwnProperty.call(extra, key)) {
        throw new Error(
          "Extra keys detected: " + Object.keys(extra).join(", ")
        );
      }
    }
    const localPath = path.relative(this._config.projectRoot, filePath);
    const partialKey = stableHash([
      this._baseHash,
      path.sep === "/" ? localPath : localPath.replaceAll(path.sep, "/"),
      customTransformOptions,
      dev,
      experimentalImportSupport,
      hot,
      inlinePlatform,
      inlineRequires,
      minify,
      nonInlinedRequires,
      platform,
      type,
      unstable_disableES6Transforms,
      unstable_memoizeInlineRequires,
      unstable_nonMemoizedInlineRequires,
      unstable_transformProfile,
    ]);
    let sha1;
    let content;
    if (fileBuffer) {
      sha1 = _crypto.default
        .createHash("sha1")
        .update(fileBuffer)
        .digest("hex");
      content = fileBuffer;
    } else {
      const result = await this._getSha1(filePath);
      if (typeof result === "string") {
        sha1 = result;
      } else {
        sha1 = result.sha1;
        if (result.content) {
          content = result.content;
        }
      }
    }
    let fullKey = Buffer.concat([partialKey, Buffer.from(sha1, "hex")]);
    let result;
    try {
      result = await cache.get(fullKey);
    } catch (error) {
      this._config.reporter.update({
        type: "cache_read_error",
        error,
      });
      throw error;
    }
    const data = result
      ? {
          result,
          sha1,
        }
      : await this._workerFarm.transform(
          localPath,
          transformerOptions,
          content
        );
    if (sha1 !== data.sha1) {
      fullKey = Buffer.concat([partialKey, Buffer.from(data.sha1, "hex")]);
    }
    cache.set(fullKey, data.result).catch((error) => {
      this._config.reporter.update({
        type: "cache_write_error",
        error,
      });
    });
    return {
      ...data.result,
      unstable_transformResultKey: fullKey.toString(),
      getSource() {
        if (fileBuffer) {
          return fileBuffer;
        }
        return fs.readFileSync(filePath);
      },
    };
  }
  async end() {
    await this._workerFarm.kill();
  }
}
function verifyRootExists(root) {
  assert(fs.statSync(root).isDirectory(), "Root has to be a valid directory");
}
module.exports = Transformer;
