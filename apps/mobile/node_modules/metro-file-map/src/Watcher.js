"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.Watcher = void 0;
var _node = _interopRequireDefault(require("./crawlers/node"));
var _watchman = _interopRequireDefault(require("./crawlers/watchman"));
var _common = require("./watchers/common");
var _FallbackWatcher = _interopRequireDefault(
  require("./watchers/FallbackWatcher"),
);
var _NativeWatcher = _interopRequireDefault(
  require("./watchers/NativeWatcher"),
);
var _WatchmanWatcher = _interopRequireDefault(
  require("./watchers/WatchmanWatcher"),
);
var _events = _interopRequireDefault(require("events"));
var fs = _interopRequireWildcard(require("fs"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
var path = _interopRequireWildcard(require("path"));
var _perf_hooks = require("perf_hooks");
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
const debug = require("debug")("Metro:Watcher");
const MAX_WAIT_TIME = 240000;
let nextInstanceId = 0;
class Watcher extends _events.default {
  _backends = [];
  _nextHealthCheckId = 0;
  _pendingHealthChecks = new Map();
  constructor(options) {
    super();
    this._options = options;
    this._instanceId = nextInstanceId++;
  }
  async crawl() {
    this._options.perfLogger?.point("crawl_start");
    const options = this._options;
    const ignoreForCrawl = (filePath) =>
      options.ignoreForCrawl(filePath) ||
      path.basename(filePath).startsWith(this._options.healthCheckFilePrefix);
    const crawl = options.useWatchman ? _watchman.default : _node.default;
    let crawler = crawl === _watchman.default ? "watchman" : "node";
    options.abortSignal.throwIfAborted();
    const crawlerOptions = {
      abortSignal: options.abortSignal,
      computeSha1: options.computeSha1,
      console: options.console,
      includeSymlinks: options.enableSymlinks,
      extensions: options.extensions,
      forceNodeFilesystemAPI: options.forceNodeFilesystemAPI,
      ignore: ignoreForCrawl,
      onStatus: (status) => {
        this.emit("status", status);
      },
      perfLogger: options.perfLogger,
      previousState: options.previousState,
      rootDir: options.rootDir,
      roots: options.roots,
    };
    const retry = (error) => {
      if (crawl === _watchman.default) {
        crawler = "node";
        options.console.warn(
          "metro-file-map: Watchman crawl failed. Retrying once with node " +
            "crawler.\n" +
            "  Usually this happens when watchman isn't running. Create an " +
            "empty `.watchmanconfig` file in your project's root folder or " +
            "initialize a git or hg repository in your project.\n" +
            "  " +
            error.toString(),
        );
        return (0, _node.default)(crawlerOptions).catch((e) => {
          throw new Error(
            "Crawler retry failed:\n" +
              `  Original error: ${error.message}\n` +
              `  Retry error: ${e.message}\n`,
          );
        });
      }
      throw error;
    };
    const logEnd = (delta) => {
      debug(
        'Crawler "%s" returned %d added/modified, %d removed, %d clock(s).',
        crawler,
        delta.changedFiles.size,
        delta.removedFiles.size,
        delta.clocks?.size ?? 0,
      );
      this._options.perfLogger?.point("crawl_end");
      return delta;
    };
    debug('Beginning crawl with "%s".', crawler);
    try {
      return crawl(crawlerOptions).catch(retry).then(logEnd);
    } catch (error) {
      return retry(error).then(logEnd);
    }
  }
  async watch(onChange) {
    const { extensions, ignorePatternForWatch, useWatchman } = this._options;
    const WatcherImpl = useWatchman
      ? _WatchmanWatcher.default
      : _NativeWatcher.default.isSupported()
        ? _NativeWatcher.default
        : _FallbackWatcher.default;
    let watcher = "fallback";
    if (WatcherImpl === _WatchmanWatcher.default) {
      watcher = "watchman";
    } else if (WatcherImpl === _NativeWatcher.default) {
      watcher = "native";
    }
    debug(`Using watcher: ${watcher}`);
    this._options.perfLogger?.annotate({
      string: {
        watcher,
      },
    });
    this._activeWatcher = watcher;
    const createWatcherBackend = (root) => {
      const watcherOptions = {
        dot: true,
        globs: [
          "**/package.json",
          "**/" + this._options.healthCheckFilePrefix + "*",
          ...extensions.map((extension) => "**/*." + extension),
        ],
        ignored: ignorePatternForWatch,
        watchmanDeferStates: this._options.watchmanDeferStates,
      };
      const watcher = new WatcherImpl(root, watcherOptions);
      return new Promise(async (resolve, reject) => {
        const rejectTimeout = setTimeout(
          () => reject(new Error("Failed to start watch mode.")),
          MAX_WAIT_TIME,
        );
        watcher.onFileEvent((change) => {
          const basename = path.basename(change.relativePath);
          if (basename.startsWith(this._options.healthCheckFilePrefix)) {
            if (change.event === _common.TOUCH_EVENT) {
              debug(
                "Observed possible health check cookie: %s in %s",
                change.relativePath,
                root,
              );
              this._handleHealthCheckObservation(basename);
            }
            return;
          }
          onChange(change);
        });
        await watcher.startWatching();
        clearTimeout(rejectTimeout);
        resolve(watcher);
      });
    };
    this._backends = await Promise.all(
      this._options.roots.map(createWatcherBackend),
    );
  }
  _handleHealthCheckObservation(basename) {
    const resolveHealthCheck = this._pendingHealthChecks.get(basename);
    if (!resolveHealthCheck) {
      return;
    }
    resolveHealthCheck();
  }
  async close() {
    await Promise.all(this._backends.map((watcher) => watcher.stopWatching()));
    this._activeWatcher = null;
  }
  async checkHealth(timeout) {
    const healthCheckId = this._nextHealthCheckId++;
    if (healthCheckId === Number.MAX_SAFE_INTEGER) {
      this._nextHealthCheckId = 0;
    }
    const watcher = this._activeWatcher;
    const basename =
      this._options.healthCheckFilePrefix +
      "-" +
      process.pid +
      "-" +
      this._instanceId +
      "-" +
      healthCheckId;
    const healthCheckPath = path.join(this._options.rootDir, basename);
    let result;
    const timeoutPromise = new Promise((resolve) =>
      setTimeout(resolve, timeout),
    ).then(() => {
      if (!result) {
        result = {
          type: "timeout",
          pauseReason: this._backends[0]?.getPauseReason(),
          timeout,
          watcher,
        };
      }
    });
    const startTime = _perf_hooks.performance.now();
    debug("Creating health check cookie: %s", healthCheckPath);
    const creationPromise = fs.promises
      .writeFile(healthCheckPath, String(startTime))
      .catch((error) => {
        if (!result) {
          result = {
            type: "error",
            error,
            timeout,
            watcher,
          };
        }
      });
    const observationPromise = new Promise((resolve) => {
      this._pendingHealthChecks.set(basename, resolve);
    }).then(() => {
      if (!result) {
        result = {
          type: "success",
          timeElapsed: _perf_hooks.performance.now() - startTime,
          timeout,
          watcher,
        };
      }
    });
    await Promise.race([
      timeoutPromise,
      creationPromise.then(() => observationPromise),
    ]);
    this._pendingHealthChecks.delete(basename);
    creationPromise.then(() =>
      fs.promises.unlink(healthCheckPath).catch(() => {}),
    );
    debug("Health check result: %o", result);
    return (0, _nullthrows.default)(result);
  }
}
exports.Watcher = Watcher;
