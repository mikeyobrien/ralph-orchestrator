"use strict";

const FileStore = require("./FileStore");
const fs = require("fs");
const path = require("path");
const walkSync = function (dir, filelist) {
  const files = fs.readdirSync(dir);
  filelist = filelist || [];
  files.forEach(function (file) {
    const fullPath = path.join(dir, file);
    const stats = fs.statSync(fullPath);
    if (stats.isDirectory()) {
      filelist = walkSync(fullPath + path.sep, filelist);
    } else {
      filelist.push({
        path: fullPath,
        stats,
      });
    }
  });
  return filelist;
};
function get(property, defaultValue) {
  if (property == null) {
    return defaultValue;
  }
  return property;
}
class AutoCleanFileStore extends FileStore {
  constructor(opts) {
    super({
      root: opts.root,
    });
    this._intervalMs = get(opts.intervalMs, 10 * 60 * 1000);
    this._cleanupThresholdMs = get(
      opts.cleanupThresholdMs,
      3 * 24 * 60 * 60 * 1000
    );
    this._scheduleCleanup();
  }
  _scheduleCleanup() {
    setTimeout(this._doCleanup.bind(this), this._intervalMs);
  }
  _doCleanup() {
    const files = walkSync(this._root, []);
    let warned = false;
    files.forEach((file) => {
      if (file.stats.mtimeMs < Date.now() - this._cleanupThresholdMs) {
        try {
          fs.unlinkSync(file.path);
        } catch (e) {
          if (!warned) {
            console.warn(
              "Problem cleaning up cache for " + file.path + ": " + e.message
            );
            warned = true;
          }
        }
      }
    });
    this._scheduleCleanup();
  }
}
module.exports = AutoCleanFileStore;
