"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.AbstractWatcher = void 0;
var _common = require("./common");
var _events = _interopRequireDefault(require("events"));
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
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class AbstractWatcher {
  #emitter = new _events.default();
  constructor(dir, { ignored, globs, dot }) {
    this.dot = dot || false;
    this.ignored = ignored;
    this.globs = globs;
    this.doIgnore = ignored
      ? (filePath) => (0, _common.posixPathMatchesPattern)(ignored, filePath)
      : () => false;
    this.root = path.resolve(dir);
  }
  onFileEvent(listener) {
    this.#emitter.on("fileevent", listener);
    return () => {
      this.#emitter.removeListener("fileevent", listener);
    };
  }
  onError(listener) {
    this.#emitter.on("error", listener);
    return () => {
      this.#emitter.removeListener("error", listener);
    };
  }
  async startWatching() {}
  async stopWatching() {
    this.#emitter.removeAllListeners();
  }
  emitFileEvent(event) {
    this.#emitter.emit("fileevent", {
      ...event,
      root: this.root,
    });
  }
  emitError(error) {
    this.#emitter.emit("error", error);
  }
  getPauseReason() {
    return null;
  }
}
exports.AbstractWatcher = AbstractWatcher;
