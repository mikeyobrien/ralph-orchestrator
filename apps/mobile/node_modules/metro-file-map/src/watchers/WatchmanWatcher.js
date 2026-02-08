"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _normalizePathSeparatorsToSystem = _interopRequireDefault(
  require("../lib/normalizePathSeparatorsToSystem"),
);
var _AbstractWatcher = require("./AbstractWatcher");
var common = _interopRequireWildcard(require("./common"));
var _RecrawlWarning = _interopRequireDefault(require("./RecrawlWarning"));
var _assert = _interopRequireDefault(require("assert"));
var _crypto = require("crypto");
var _fbWatchman = _interopRequireDefault(require("fb-watchman"));
var _invariant = _interopRequireDefault(require("invariant"));
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
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const debug = require("debug")("Metro:WatchmanWatcher");
const DELETE_EVENT = common.DELETE_EVENT;
const TOUCH_EVENT = common.TOUCH_EVENT;
const SUB_PREFIX = "metro-file-map";
class WatchmanWatcher extends _AbstractWatcher.AbstractWatcher {
  #deferringStates = null;
  constructor(dir, { watchmanDeferStates, ...opts }) {
    super(dir, opts);
    this.watchmanDeferStates = watchmanDeferStates;
    const watchKey = (0, _crypto.createHash)("md5")
      .update(this.root)
      .digest("hex");
    const readablePath = this.root
      .replace(/[\/\\]/g, "-")
      .replace(/[^\-\w]/g, "");
    this.subscriptionName = `${SUB_PREFIX}-${process.pid}-${readablePath}-${watchKey}`;
  }
  async startWatching() {
    await new Promise((resolve, reject) => this._init(resolve, reject));
  }
  _init(onReady, onError) {
    if (this.client) {
      this.client.removeAllListeners();
    }
    const self = this;
    this.client = new _fbWatchman.default.Client();
    this.client.on("error", (error) => {
      this.emitError(error);
    });
    this.client.on("subscription", (changeEvent) =>
      this._handleChangeEvent(changeEvent),
    );
    this.client.on("end", () => {
      console.warn(
        "[metro-file-map] Warning: Lost connection to Watchman, reconnecting..",
      );
      self._init(
        () => {},
        (error) => self.emitError(error),
      );
    });
    this.watchProjectInfo = null;
    function getWatchRoot() {
      return self.watchProjectInfo ? self.watchProjectInfo.root : self.root;
    }
    function onWatchProject(error, resp) {
      if (error) {
        onError(error);
        return;
      }
      debug("Received watch-project response: %s", resp.relative_path);
      handleWarning(resp);
      self.watchProjectInfo = {
        relativePath: resp.relative_path
          ? (0, _normalizePathSeparatorsToSystem.default)(resp.relative_path)
          : "",
        root: (0, _normalizePathSeparatorsToSystem.default)(resp.watch),
      };
      self.client.command(["clock", getWatchRoot()], onClock);
    }
    function onClock(error, resp) {
      if (error) {
        onError(error);
        return;
      }
      debug("Received clock response: %s", resp.clock);
      const watchProjectInfo = self.watchProjectInfo;
      (0, _invariant.default)(
        watchProjectInfo != null,
        "watch-project response should have been set before clock response",
      );
      handleWarning(resp);
      const options = {
        fields: ["name", "exists", "new", "type", "size", "mtime_ms"],
        since: resp.clock,
        defer: self.watchmanDeferStates,
        relative_root: watchProjectInfo.relativePath,
      };
      if (self.globs.length === 0 && !self.dot) {
        options.expression = [
          "match",
          "**",
          "wholename",
          {
            includedotfiles: false,
          },
        ];
      }
      self.client.command(
        ["subscribe", getWatchRoot(), self.subscriptionName, options],
        onSubscribe,
      );
    }
    const onSubscribe = (error, resp) => {
      if (error) {
        onError(error);
        return;
      }
      debug("Received subscribe response: %s", resp.subscribe);
      handleWarning(resp);
      if (resp["asserted-states"] != null) {
        this.#deferringStates = new Set(resp["asserted-states"]);
      }
      onReady();
    };
    self.client.command(["watch-project", getWatchRoot()], onWatchProject);
  }
  _handleChangeEvent(resp) {
    debug(
      "Received subscription response: %s (fresh: %s, files: %s, enter: %s, leave: %s, clock: %s)",
      resp.subscription,
      resp.is_fresh_instance,
      resp.files?.length,
      resp["state-enter"],
      resp["state-leave"],
      resp.clock,
    );
    _assert.default.equal(
      resp.subscription,
      this.subscriptionName,
      "Invalid subscription event.",
    );
    if (Array.isArray(resp.files)) {
      resp.files.forEach((change) =>
        this._handleFileChange(change, resp.clock),
      );
    }
    const { "state-enter": stateEnter, "state-leave": stateLeave } = resp;
    if (
      stateEnter != null &&
      (this.watchmanDeferStates ?? []).includes(stateEnter)
    ) {
      this.#deferringStates?.add(stateEnter);
      debug(
        'Watchman reports "%s" just started. Filesystem notifications are paused.',
        stateEnter,
      );
    }
    if (
      stateLeave != null &&
      (this.watchmanDeferStates ?? []).includes(stateLeave)
    ) {
      this.#deferringStates?.delete(stateLeave);
      debug(
        'Watchman reports "%s" ended. Filesystem notifications resumed.',
        stateLeave,
      );
    }
  }
  _handleFileChange(changeDescriptor, rawClock) {
    const self = this;
    const watchProjectInfo = self.watchProjectInfo;
    (0, _invariant.default)(
      watchProjectInfo != null,
      "watch-project response should have been set before receiving subscription events",
    );
    const {
      name: relativePosixPath,
      new: isNew = false,
      exists = false,
      type,
      mtime_ms,
      size,
    } = changeDescriptor;
    const relativePath = (0, _normalizePathSeparatorsToSystem.default)(
      relativePosixPath,
    );
    debug(
      "Handling change to: %s (new: %s, exists: %s, type: %s)",
      relativePath,
      isNew,
      exists,
      type,
    );
    if (type != null && !(type === "f" || type === "d" || type === "l")) {
      return;
    }
    if (
      this.doIgnore(relativePath) ||
      !common.includedByGlob(type, this.globs, this.dot, relativePath)
    ) {
      return;
    }
    const clock =
      typeof rawClock === "string" && this.watchProjectInfo != null
        ? [this.watchProjectInfo.root, rawClock]
        : undefined;
    if (!exists) {
      self.emitFileEvent({
        event: DELETE_EVENT,
        clock,
        relativePath,
      });
    } else {
      (0, _invariant.default)(
        type != null && mtime_ms != null && size != null,
        'Watchman file change event for "%s" missing some requested metadata. ' +
          "Got type: %s, mtime_ms: %s, size: %s",
        relativePath,
        type,
        mtime_ms,
        size,
      );
      if (!(type === "d" && !isNew)) {
        const mtime = Number(mtime_ms);
        self.emitFileEvent({
          event: TOUCH_EVENT,
          clock,
          relativePath,
          metadata: {
            modifiedTime: mtime !== 0 ? mtime : null,
            size,
            type,
          },
        });
      }
    }
  }
  async stopWatching() {
    await super.stopWatching();
    if (this.client) {
      this.client.removeAllListeners();
      this.client.end();
    }
    this.#deferringStates = null;
  }
  getPauseReason() {
    if (this.#deferringStates == null || this.#deferringStates.size === 0) {
      return null;
    }
    const states = [...this.#deferringStates];
    if (states.length === 1) {
      return `The watch is in the '${states[0]}' state.`;
    }
    return `The watch is in the ${states
      .slice(0, -1)
      .map((s) => `'${s}'`)
      .join(", ")} and '${states[states.length - 1]}' states.`;
  }
}
exports.default = WatchmanWatcher;
function handleWarning(resp) {
  if ("warning" in resp) {
    if (_RecrawlWarning.default.isRecrawlWarningDupe(resp.warning)) {
      return true;
    }
    console.warn(resp.warning);
    return true;
  } else {
    return false;
  }
}
