"use strict";

const { AbstractWatcher } = require("./AbstractWatcher");
const common = require("./common");
const fs = require("fs");
const platform = require("os").platform();
const path = require("path");
const walker = require("walker");
const fsPromises = fs.promises;
const TOUCH_EVENT = common.TOUCH_EVENT;
const DELETE_EVENT = common.DELETE_EVENT;
const DEBOUNCE_MS = 100;
module.exports = class FallbackWatcher extends AbstractWatcher {
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
        this.ignored
      );
    });
  }
  _register(filepath, type) {
    const dir = path.dirname(filepath);
    const filename = path.basename(filepath);
    if (this._dirRegistry[dir] && this._dirRegistry[dir][filename]) {
      return false;
    }
    const relativePath = path.relative(this.root, filepath);
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
    const dir = path.dirname(filepath);
    if (this._dirRegistry[dir]) {
      const filename = path.basename(filepath);
      delete this._dirRegistry[dir][filename];
    }
  }
  _unregisterDir(dirpath) {
    if (this._dirRegistry[dirpath]) {
      delete this._dirRegistry[dirpath];
    }
  }
  _registered(fullpath) {
    const dir = path.dirname(fullpath);
    return !!(
      this._dirRegistry[fullpath] ||
      (this._dirRegistry[dir] &&
        this._dirRegistry[dir][path.basename(fullpath)])
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
    const watcher = fs.watch(
      dir,
      {
        persistent: true,
      },
      (event, filename) => this._normalizeChange(dir, event, filename)
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
      this._stopWatching(dir)
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
      fs.lstat(path.join(dir, file), (error, stat) => {
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
            this.emitError(error)
          );
        }
      });
    } else {
      this._processChange(dir, event, path.normalize(file)).catch((error) =>
        this.emitError(error)
      );
    }
  }
  async _processChange(dir, event, file) {
    const fullPath = path.join(dir, file);
    const relativePath = path.join(path.relative(this.root, dir), file);
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
          path.resolve(this.root, relativePath),
          (dir, stats) => {
            if (this._watchdir(dir)) {
              this._emitEvent({
                event: TOUCH_EVENT,
                relativePath: path.relative(this.root, dir),
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
                relativePath: path.relative(this.root, file),
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
                relativePath: path.relative(this.root, symlink),
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
          this.ignored
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
      }, DEBOUNCE_MS)
    );
  }
  getPauseReason() {
    return null;
  }
};
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
  ignored
) {
  const walk = walker(dir);
  if (ignored) {
    walk.filterDir(
      (currentDir) => !common.posixPathMatchesPattern(ignored, currentDir)
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
  return (filepath, stats) => callback(path.normalize(filepath), stats);
}
