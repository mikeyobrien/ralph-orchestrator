"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _pathUtils = require("../lib/pathUtils");
var _getTransformCacheKey = _interopRequireDefault(
  require("./getTransformCacheKey"),
);
var _WorkerFarm = _interopRequireDefault(require("./WorkerFarm"));
var _assert = _interopRequireDefault(require("assert"));
var _crypto = _interopRequireDefault(require("crypto"));
var _fs = _interopRequireDefault(require("fs"));
var _metroCache = require("metro-cache");
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const debug = require("debug")("Metro:Transformer");
class Transformer {
  constructor(config, opts) {
    this._config = config;
    this._config.watchFolders.forEach(verifyRootExists);
    this._cache = new _metroCache.Cache(config.cacheStores);
    this._getSha1 = opts.getOrComputeSha1;
    const {
      getTransformOptions: _getTransformOptions,
      transformVariants: _transformVariants,
      unstable_workerThreads: _workerThreads,
      ...transformerConfig
    } = this._config.transformer;
    const transformerOptions = {
      transformerPath: this._config.transformerPath,
      transformerConfig,
    };
    this._workerFarm = new _WorkerFarm.default(config, transformerOptions);
    const globalCacheKey = this._cache.isDisabled
      ? ""
      : (0, _getTransformCacheKey.default)({
          cacheVersion: this._config.cacheVersion,
          projectRoot: this._config.projectRoot,
          transformerConfig: transformerOptions,
        });
    const baseHashBuffer = (0, _metroCache.stableHash)([globalCacheKey]);
    this._baseHash = baseHashBuffer.toString("binary");
    debug("Base hash: %s", baseHashBuffer.toString("hex"));
  }
  async transformFile(filePath, transformerOptions, fileBuffer) {
    const cache = this._cache;
    const {
      customTransformOptions,
      dev,
      experimentalImportSupport,
      inlinePlatform,
      inlineRequires,
      minify,
      nonInlinedRequires,
      platform,
      type,
      unstable_transformProfile,
      unstable_memoizeInlineRequires,
      unstable_nonMemoizedInlineRequires,
      ...extra
    } = transformerOptions;
    for (const key in extra) {
      if (hasOwnProperty.call(extra, key)) {
        throw new Error(
          "Extra keys detected: " + Object.keys(extra).join(", "),
        );
      }
    }
    const localPath = _path.default.relative(
      this._config.projectRoot,
      filePath,
    );
    const partialKey = (0, _metroCache.stableHash)([
      this._baseHash,
      (0, _pathUtils.normalizePathSeparatorsToPosix)(localPath),
      customTransformOptions,
      dev,
      experimentalImportSupport,
      inlinePlatform,
      inlineRequires,
      minify,
      nonInlinedRequires,
      platform,
      type,
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
      sha1 = result.sha1;
      if (result.content) {
        content = result.content;
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
          content,
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
        return _fs.default.readFileSync(filePath);
      },
    };
  }
  async end() {
    await this._workerFarm.kill();
  }
}
exports.default = Transformer;
function verifyRootExists(root) {
  (0, _assert.default)(
    _fs.default.statSync(root).isDirectory(),
    "Root has to be a valid directory",
  );
}
