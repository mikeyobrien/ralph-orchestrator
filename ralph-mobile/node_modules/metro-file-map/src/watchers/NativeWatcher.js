"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _AbstractWatcher = require("./AbstractWatcher");
var _common = require("./common");
var _fs = require("fs");
var _os = require("os");
var path = _interopRequireWildcard(require("path"));
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
  return (n.default = e), t && t.set(e, n), n;
}
const debug = require("debug")("Metro:NativeWatcher");
const TOUCH_EVENT = "touch";
const DELETE_EVENT = "delete";
class NativeWatcher extends _AbstractWatcher.AbstractWatcher {
  #fsWatcher;
  static isSupported() {
    return (0, _os.platform)() === "darwin";
  }
  constructor(dir, opts) {
    if (!NativeWatcher.isSupported) {
      throw new Error("This watcher can only be used on macOS");
    }
    super(dir, opts);
  }
  async startWatching() {
    this.#fsWatcher = (0, _fs.watch)(
      this.root,
      {
        persistent: false,
        recursive: true,
      },
      (_event, relativePath) => {
        this._handleEvent(relativePath).catch((error) => {
          this.emitError(error);
        });
      }
    );
    debug("Watching %s", this.root);
  }
  async stopWatching() {
    await super.stopWatching();
    if (this.#fsWatcher) {
      this.#fsWatcher.close();
    }
  }
  async _handleEvent(relativePath) {
    const absolutePath = path.resolve(this.root, relativePath);
    if (this.doIgnore(relativePath)) {
      debug("Ignoring event on %s (root: %s)", relativePath, this.root);
      return;
    }
    debug("Handling event on %s (root: %s)", relativePath, this.root);
    try {
      const stat = await _fs.promises.lstat(absolutePath);
      const type = (0, _common.typeFromStat)(stat);
      if (!type) {
        return;
      }
      if (
        !(0, _common.includedByGlob)(type, this.globs, this.dot, relativePath)
      ) {
        return;
      }
      this.emitFileEvent({
        event: TOUCH_EVENT,
        relativePath,
        metadata: {
          type,
          modifiedTime: stat.mtime.getTime(),
          size: stat.size,
        },
      });
    } catch (error) {
      if (error?.code !== "ENOENT") {
        this.emitError(error);
        return;
      }
      this.emitFileEvent({
        event: DELETE_EVENT,
        relativePath,
      });
    }
  }
}
exports.default = NativeWatcher;
