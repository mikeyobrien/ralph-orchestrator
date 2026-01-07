"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.DiskCacheManager = void 0;
var _rootRelativeCacheKeys = _interopRequireDefault(
  require("../lib/rootRelativeCacheKeys")
);
var _fs = require("fs");
var _os = require("os");
var _path = _interopRequireDefault(require("path"));
var _timers = require("timers");
var _v = require("v8");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const debug = require("debug")("Metro:FileMapCache");
const DEFAULT_PREFIX = "metro-file-map";
const DEFAULT_DIRECTORY = (0, _os.tmpdir)();
const DEFAULT_AUTO_SAVE_DEBOUNCE_MS = 5000;
class DiskCacheManager {
  #autoSaveOpts;
  #cachePath;
  #debounceTimeout = null;
  #writePromise = Promise.resolve();
  #hasUnwrittenChanges = false;
  #tryWrite;
  #stopListening;
  constructor(
    { buildParameters },
    { autoSave = {}, cacheDirectory, cacheFilePrefix }
  ) {
    this.#cachePath = DiskCacheManager.getCacheFilePath(
      buildParameters,
      cacheFilePrefix,
      cacheDirectory
    );
    if (autoSave) {
      const { debounceMs = DEFAULT_AUTO_SAVE_DEBOUNCE_MS } =
        autoSave === true ? {} : autoSave;
      this.#autoSaveOpts = {
        debounceMs,
      };
    }
  }
  static getCacheFilePath(buildParameters, cacheFilePrefix, cacheDirectory) {
    const { rootDirHash, relativeConfigHash } = (0,
    _rootRelativeCacheKeys.default)(buildParameters);
    return _path.default.join(
      cacheDirectory ?? DEFAULT_DIRECTORY,
      `${
        cacheFilePrefix ?? DEFAULT_PREFIX
      }-${rootDirHash}-${relativeConfigHash}`
    );
  }
  getCacheFilePath() {
    return this.#cachePath;
  }
  async read() {
    try {
      return (0, _v.deserialize)(await _fs.promises.readFile(this.#cachePath));
    } catch (e) {
      if (e?.code === "ENOENT") {
        return null;
      }
      throw e;
    }
  }
  async write(
    getSnapshot,
    { changedSinceCacheRead, eventSource, onWriteError }
  ) {
    const tryWrite = (this.#tryWrite = () => {
      this.#writePromise = this.#writePromise
        .then(async () => {
          if (!this.#hasUnwrittenChanges) {
            return;
          }
          const data = getSnapshot();
          this.#hasUnwrittenChanges = false;
          await _fs.promises.writeFile(
            this.#cachePath,
            (0, _v.serialize)(data)
          );
          debug("Written cache to %s", this.#cachePath);
        })
        .catch(onWriteError);
      return this.#writePromise;
    });
    if (this.#autoSaveOpts) {
      const autoSave = this.#autoSaveOpts;
      this.#stopListening?.();
      this.#stopListening = eventSource.onChange(() => {
        this.#hasUnwrittenChanges = true;
        if (this.#debounceTimeout) {
          this.#debounceTimeout.refresh();
        } else {
          this.#debounceTimeout = (0, _timers.setTimeout)(
            () => tryWrite(),
            autoSave.debounceMs
          ).unref();
        }
      });
    }
    if (changedSinceCacheRead) {
      this.#hasUnwrittenChanges = true;
      await tryWrite();
    }
  }
  async end() {
    if (this.#debounceTimeout) {
      (0, _timers.clearTimeout)(this.#debounceTimeout);
    }
    this.#stopListening?.();
    await this.#tryWrite?.();
  }
}
exports.DiskCacheManager = DiskCacheManager;
