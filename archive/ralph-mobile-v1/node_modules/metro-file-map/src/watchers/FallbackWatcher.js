"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _AbstractWatcher = require("./AbstractWatcher");
var common = _interopRequireWildcard(require("./common"));
var _fs = _interopRequireDefault(require("fs"));
var _os = _interopRequireDefault(require("os"));
var _path = _interopRequireDefault(require("path"));
var _walker = _interopRequireDefault(require("walker"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function _getRequireWildcardCache(e) {
  if ("function" != typeof WeakMap) return null;
  var r = new WeakMap(),
    t = new WeakMap();
  return (_getRequireWildcardCache = function (e) {
    return e ? t : r;
  })(e);
}
function _interopRequireWildcard(e, r) {
  if (!r && e && e.__esModule) return e;
  if (null === e || ("object" != typeof e && "function" != typeof e))
    return { default: e };
  var t = _getRequireWildcardCache(r);
  if (t && t.has(e)) return t.get(e);
  var n = { __proto__: null },
    a = Object.defineProperty && Object.getOwnPropertyDescriptor;
  for (var u in e)
    if ("default" !== u && {}.hasOwnProperty.call(e, u)) {
      var i = a ? Object.getOwnPropertyDescriptor(e, u) : null;
      i && (i.get || i.set) ? Object.defineProperty(n, u, i) : (n[u] = e[u]);
    }
  return ((n.default = e), t && t.set(e, n), n);
}
const platform = _os.default.platform();
const fsPromises = _fs.default.promises;
const TOUCH_EVENT = common.TOUCH_EVENT;
const DELETE_EVENT = common.DELETE_EVENT;
const DEBOUNCE_MS = 100;
class FallbackWatcher extends _AbstractWatcher.AbstractWatcher {
  _changeTimers = new Map();
  _dirRegistry = Object.create(null);
  watched = Object.create(null);
  async startWatching() {
    this._watchdir(this.root);
    await new Promise((resolve) => {
      recReaddir(
        this.root,
        (dir) => {
          this._watchdir(dir);
        },
        (filename) => {
          this._register(filename, "f");
        },
        (symlink) => {
          this._register(symlink, "l");
        },
        () => {
          resolve();
        },
        this._checkedEmitError,
        this.ignored,
      );
    });
  }
  _register(filepath, type) {
    const dir = _path.default.dirname(filepath);
    const filename = _path.default.basename(filepath);
    if (this._dirRegistry[dir] && this._dirRegistry[dir][filename]) {
      return false;
    }
    const relativePath = _path.default.relative(this.root, filepath);
    if (
      this.doIgnore(relativePath) ||
      (type === "f" &&
        !common.includedByGlob("f", this.globs, this.dot, relativePath))
    ) {
      return false;
    }
    if (!this._dirRegistry[dir]) {
      this._dirRegistry[dir] = Object.create(null);
    }
    this._dirRegistry[dir][filename] = true;
    return true;
  }
  _unregister(filepath) {
    const dir = _path.default.dirname(filepath);
    if (this._dirRegistry[dir]) {
      const filename = _path.default.basename(filepath);
      delete this._dirRegistry[dir][filename];
    }
  }
  _unregisterDir(dirpath) {
    if (this._dirRegistry[dirpath]) {
      delete this._dirRegistry[dirpath];
    }
  }
  _registered(fullpath) {
    const dir = _path.default.dirname(fullpath);
    return !!(
      this._dirRegistry[fullpath] ||
      (this._dirRegistry[dir] &&
        this._dirRegistry[dir][_path.default.basename(fullpath)])
    );
  }
  _checkedEmitError = (error) => {
    if (!isIgnorableFileError(error)) {
      this.emitError(error);
    }
  };
  _watchdir = (dir) => {
    if (this.watched[dir]) {
      return false;
    }
    const watcher = _fs.default.watch(
      dir,
      {
        persistent: true,
      },
      (event, filename) => this._normalizeChange(dir, event, filename),
    );
    this.watched[dir] = watcher;
    watcher.on("error", this._checkedEmitError);
    if (this.root !== dir) {
      this._register(dir, "d");
    }
    return true;
  };
  async _stopWatching(dir) {
    if (this.watched[dir]) {
      await new Promise((resolve) => {
        this.watched[dir].once("close", () => process.nextTick(resolve));
        this.watched[dir].close();
        delete this.watched[dir];
      });
    }
  }
  async stopWatching() {
    await super.stopWatching();
    const promises = Object.keys(this.watched).map((dir) =>
      this._stopWatching(dir),
    );
    await Promise.all(promises);
  }
  _detectChangedFile(dir, event, callback) {
    if (!this._dirRegistry[dir]) {
      return;
    }
    let found = false;
    let closest = null;
    let c = 0;
    Object.keys(this._dirRegistry[dir]).forEach((file, i, arr) => {
      _fs.default.lstat(_path.default.join(dir, file), (error, stat) => {
        if (found) {
          return;
        }
        if (error) {
          if (isIgnorableFileError(error)) {
            found = true;
            callback(file);
          } else {
            this.emitError(error);
          }
        } else {
          if (closest == null || stat.mtime > closest.mtime) {
            closest = {
              file,
              mtime: stat.mtime,
            };
          }
          if (arr.length === ++c) {
            callback(closest.file);
          }
        }
      });
    });
  }
  _normalizeChange(dir, event, file) {
    if (!file) {
      this._detectChangedFile(dir, event, (actualFile) => {
        if (actualFile) {
          this._processChange(dir, event, actualFile).catch((error) =>
            this.emitError(error),
          );
        }
      });
    } else {
      this._processChange(dir, event, _path.default.normalize(file)).catch(
        (error) => this.emitError(error),
      );
    }
  }
  async _processChange(dir, event, file) {
    const fullPath = _path.default.join(dir, file);
    const relativePath = _path.default.join(
      _path.default.relative(this.root, dir),
      file,
    );
    const registered = this._registered(fullPath);
    try {
      const stat = await fsPromises.lstat(fullPath);
      if (stat.isDirectory()) {
        if (event === "change") {
          return;
        }
        if (
          this.doIgnore(relativePath) ||
          !common.includedByGlob("d", this.globs, this.dot, relativePath)
        ) {
          return;
        }
        recReaddir(
          _path.default.resolve(this.root, relativePath),
          (dir, stats) => {
            if (this._watchdir(dir)) {
              this._emitEvent({
                event: TOUCH_EVENT,
                relativePath: _path.default.relative(this.root, dir),
                metadata: {
                  modifiedTime: stats.mtime.getTime(),
                  size: stats.size,
                  type: "d",
                },
              });
            }
          },
          (file, stats) => {
            if (this._register(file, "f")) {
              this._emitEvent({
                event: TOUCH_EVENT,
                relativePath: _path.default.relative(this.root, file),
                metadata: {
                  modifiedTime: stats.mtime.getTime(),
                  size: stats.size,
                  type: "f",
                },
              });
            }
          },
          (symlink, stats) => {
            if (this._register(symlink, "l")) {
              this.emitFileEvent({
                event: TOUCH_EVENT,
                relativePath: _path.default.relative(this.root, symlink),
                metadata: {
                  modifiedTime: stats.mtime.getTime(),
                  size: stats.size,
                  type: "l",
                },
              });
            }
          },
          function endCallback() {},
          this._checkedEmitError,
          this.ignored,
        );
      } else {
        const type = common.typeFromStat(stat);
        if (type == null) {
          return;
        }
        const metadata = {
          modifiedTime: stat.mtime.getTime(),
          size: stat.size,
          type,
        };
        if (registered) {
          this._emitEvent({
            event: TOUCH_EVENT,
            relativePath,
            metadata,
          });
        } else {
          if (this._register(fullPath, type)) {
            this._emitEvent({
              event: TOUCH_EVENT,
              relativePath,
              metadata,
            });
          }
        }
      }
    } catch (error) {
      if (!isIgnorableFileError(error)) {
        this.emitError(error);
        return;
      }
      this._unregister(fullPath);
      this._unregisterDir(fullPath);
      if (registered) {
        this._emitEvent({
          event: DELETE_EVENT,
          relativePath,
        });
      }
      await this._stopWatching(fullPath);
    }
  }
  _emitEvent(change) {
    const { event, relativePath } = change;
    const key = event + "-" + relativePath;
    const existingTimer = this._changeTimers.get(key);
    if (existingTimer) {
      clearTimeout(existingTimer);
    }
    this._changeTimers.set(
      key,
      setTimeout(() => {
        this._changeTimers.delete(key);
        this.emitFileEvent(change);
      }, DEBOUNCE_MS),
    );
  }
  getPauseReason() {
    return null;
  }
}
exports.default = FallbackWatcher;
function isIgnorableFileError(error) {
  return (
    error.code === "ENOENT" || (error.code === "EPERM" && platform === "win32")
  );
}
function recReaddir(
  dir,
  dirCallback,
  fileCallback,
  symlinkCallback,
  endCallback,
  errorCallback,
  ignored,
) {
  const walk = (0, _walker.default)(dir);
  if (ignored) {
    walk.filterDir(
      (currentDir) => !common.posixPathMatchesPattern(ignored, currentDir),
    );
  }
  walk
    .on("dir", normalizeProxy(dirCallback))
    .on("file", normalizeProxy(fileCallback))
    .on("symlink", normalizeProxy(symlinkCallback))
    .on("error", errorCallback)
    .on("end", () => {
      if (platform === "win32") {
        setTimeout(endCallback, 1000);
      } else {
        endCallback();
      }
    });
}
function normalizeProxy(callback) {
  return (filepath, stats) =>
    callback(_path.default.normalize(filepath), stats);
}
