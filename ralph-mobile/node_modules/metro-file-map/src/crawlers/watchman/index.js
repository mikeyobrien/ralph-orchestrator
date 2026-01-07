"use strict";

var _normalizePathSeparatorsToPosix = _interopRequireDefault(
  require("../../lib/normalizePathSeparatorsToPosix")
);
var _normalizePathSeparatorsToSystem = _interopRequireDefault(
  require("../../lib/normalizePathSeparatorsToSystem")
);
var _RootPathUtils = require("../../lib/RootPathUtils");
var _planQuery = require("./planQuery");
var _invariant = _interopRequireDefault(require("invariant"));
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
  return (n.default = e), t && t.set(e, n), n;
}
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const watchman = require("fb-watchman");
const WATCHMAN_WARNING_INITIAL_DELAY_MILLISECONDS = 10000;
const WATCHMAN_WARNING_INTERVAL_MILLISECONDS = 20000;
const watchmanURL = "https://facebook.github.io/watchman/docs/troubleshooting";
function makeWatchmanError(error) {
  error.message =
    `Watchman error: ${error.message.trim()}. Make sure watchman ` +
    `is running for this project. See ${watchmanURL}.`;
  return error;
}
module.exports = async function watchmanCrawl({
  abortSignal,
  computeSha1,
  extensions,
  ignore,
  includeSymlinks,
  onStatus,
  perfLogger,
  previousState,
  rootDir,
  roots,
}) {
  abortSignal?.throwIfAborted();
  const client = new watchman.Client();
  const pathUtils = new _RootPathUtils.RootPathUtils(rootDir);
  abortSignal?.addEventListener("abort", () => client.end());
  perfLogger?.point("watchmanCrawl_start");
  const newClocks = new Map();
  let clientError;
  client.on("error", (error) => {
    clientError = makeWatchmanError(error);
  });
  const cmd = async (command, ...args) => {
    let didLogWatchmanWaitMessage = false;
    const startTime = _perf_hooks.performance.now();
    const logWatchmanWaitMessage = () => {
      didLogWatchmanWaitMessage = true;
      onStatus({
        type: "watchman_slow_command",
        timeElapsed: _perf_hooks.performance.now() - startTime,
        command,
      });
    };
    let intervalOrTimeoutId = setTimeout(() => {
      logWatchmanWaitMessage();
      intervalOrTimeoutId = setInterval(
        logWatchmanWaitMessage,
        WATCHMAN_WARNING_INTERVAL_MILLISECONDS
      );
    }, WATCHMAN_WARNING_INITIAL_DELAY_MILLISECONDS);
    try {
      const response = await new Promise((resolve, reject) =>
        client.command([command, ...args], (error, result) =>
          error ? reject(makeWatchmanError(error)) : resolve(result)
        )
      );
      if ("warning" in response) {
        onStatus({
          type: "watchman_warning",
          warning: response.warning,
          command,
        });
      }
      return response;
    } finally {
      clearInterval(intervalOrTimeoutId);
      if (didLogWatchmanWaitMessage) {
        onStatus({
          type: "watchman_slow_command_complete",
          timeElapsed: _perf_hooks.performance.now() - startTime,
          command,
        });
      }
    }
  };
  async function getWatchmanRoots(roots) {
    perfLogger?.point("watchmanCrawl/getWatchmanRoots_start");
    const watchmanRoots = new Map();
    await Promise.all(
      roots.map(async (root, index) => {
        perfLogger?.point(`watchmanCrawl/watchProject_${index}_start`);
        const response = await cmd("watch-project", root);
        perfLogger?.point(`watchmanCrawl/watchProject_${index}_end`);
        const existing = watchmanRoots.get(response.watch);
        const canBeFiltered = !existing || existing.directoryFilters.length > 0;
        if (canBeFiltered) {
          if (response.relative_path) {
            watchmanRoots.set(response.watch, {
              watcher: response.watcher,
              directoryFilters: (existing?.directoryFilters || []).concat(
                response.relative_path
              ),
            });
          } else {
            watchmanRoots.set(response.watch, {
              watcher: response.watcher,
              directoryFilters: [],
            });
          }
        }
      })
    );
    perfLogger?.point("watchmanCrawl/getWatchmanRoots_end");
    return watchmanRoots;
  }
  async function queryWatchmanForDirs(rootProjectDirMappings) {
    perfLogger?.point("watchmanCrawl/queryWatchmanForDirs_start");
    const results = new Map();
    let isFresh = false;
    await Promise.all(
      Array.from(rootProjectDirMappings).map(
        async ([posixSeparatedRoot, { directoryFilters, watcher }], index) => {
          const since = previousState.clocks.get(
            (0, _normalizePathSeparatorsToPosix.default)(
              pathUtils.absoluteToNormal(
                (0, _normalizePathSeparatorsToSystem.default)(
                  posixSeparatedRoot
                )
              )
            )
          );
          perfLogger?.annotate({
            bool: {
              [`watchmanCrawl/query_${index}_has_clock`]: since != null,
            },
          });
          const { query, queryGenerator } = (0, _planQuery.planQuery)({
            since,
            extensions,
            directoryFilters,
            includeSha1: computeSha1,
            includeSymlinks,
          });
          perfLogger?.annotate({
            string: {
              [`watchmanCrawl/query_${index}_watcher`]: watcher ?? "unknown",
              [`watchmanCrawl/query_${index}_generator`]: queryGenerator,
            },
          });
          perfLogger?.point(`watchmanCrawl/query_${index}_start`);
          const response = await cmd("query", posixSeparatedRoot, query);
          perfLogger?.point(`watchmanCrawl/query_${index}_end`);
          const isSourceControlQuery =
            typeof since !== "string" && since?.scm?.["mergebase-with"] != null;
          if (!isSourceControlQuery) {
            isFresh = isFresh || response.is_fresh_instance;
          }
          results.set(posixSeparatedRoot, response);
        }
      )
    );
    perfLogger?.point("watchmanCrawl/queryWatchmanForDirs_end");
    return {
      isFresh,
      results,
    };
  }
  let removedFiles = new Set();
  let changedFiles = new Map();
  let results;
  let isFresh = false;
  let queryError;
  try {
    const watchmanRoots = await getWatchmanRoots(roots);
    const watchmanFileResults = await queryWatchmanForDirs(watchmanRoots);
    results = watchmanFileResults.results;
    isFresh = watchmanFileResults.isFresh;
  } catch (e) {
    queryError = e;
  }
  client.end();
  if (results == null) {
    if (clientError) {
      perfLogger?.annotate({
        string: {
          "watchmanCrawl/client_error":
            clientError.message ?? "[message missing]",
        },
      });
    }
    if (queryError) {
      perfLogger?.annotate({
        string: {
          "watchmanCrawl/query_error":
            queryError.message ?? "[message missing]",
        },
      });
    }
    perfLogger?.point("watchmanCrawl_end");
    abortSignal?.throwIfAborted();
    throw (
      queryError ?? clientError ?? new Error("Watchman file results missing")
    );
  }
  perfLogger?.point("watchmanCrawl/processResults_start");
  const freshFileData = new Map();
  for (const [watchRoot, response] of results) {
    const fsRoot = (0, _normalizePathSeparatorsToSystem.default)(watchRoot);
    const relativeFsRoot = pathUtils.absoluteToNormal(fsRoot);
    newClocks.set(
      (0, _normalizePathSeparatorsToPosix.default)(relativeFsRoot),
      typeof response.clock === "string" ? response.clock : response.clock.clock
    );
    for (const fileData of response.files) {
      const filePath =
        fsRoot +
        path.sep +
        (0, _normalizePathSeparatorsToSystem.default)(fileData.name);
      const relativeFilePath = pathUtils.absoluteToNormal(filePath);
      if (!fileData.exists) {
        if (!isFresh) {
          removedFiles.add(relativeFilePath);
        }
      } else if (!ignore(filePath)) {
        const { mtime_ms, size } = fileData;
        (0, _invariant.default)(
          mtime_ms != null && size != null,
          "missing file data in watchman response"
        );
        const mtime =
          typeof mtime_ms === "number" ? mtime_ms : mtime_ms.toNumber();
        let sha1hex = fileData["content.sha1hex"];
        if (typeof sha1hex !== "string" || sha1hex.length !== 40) {
          sha1hex = undefined;
        }
        let symlinkInfo = 0;
        if (fileData.type === "l") {
          symlinkInfo = fileData["symlink_target"] ?? 1;
        }
        const nextData = ["", mtime, size, 0, "", sha1hex ?? null, symlinkInfo];
        if (isFresh) {
          freshFileData.set(relativeFilePath, nextData);
        } else {
          changedFiles.set(relativeFilePath, nextData);
        }
      }
    }
  }
  if (isFresh) {
    ({ changedFiles, removedFiles } =
      previousState.fileSystem.getDifference(freshFileData));
  }
  perfLogger?.point("watchmanCrawl/processResults_end");
  perfLogger?.point("watchmanCrawl_end");
  abortSignal?.throwIfAborted();
  return {
    changedFiles,
    removedFiles,
    clocks: newClocks,
  };
};
