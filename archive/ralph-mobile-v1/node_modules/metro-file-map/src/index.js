"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
Object.defineProperty(exports, "DiskCacheManager", {
  enumerable: true,
  get: function () {
    return _DiskCacheManager.DiskCacheManager;
  },
});
Object.defineProperty(exports, "DuplicateHasteCandidatesError", {
  enumerable: true,
  get: function () {
    return _DuplicateHasteCandidatesError.DuplicateHasteCandidatesError;
  },
});
Object.defineProperty(exports, "HasteConflictsError", {
  enumerable: true,
  get: function () {
    return _HasteConflictsError.HasteConflictsError;
  },
});
Object.defineProperty(exports, "HastePlugin", {
  enumerable: true,
  get: function () {
    return _HastePlugin.default;
  },
});
exports.default = void 0;
var _DiskCacheManager = require("./cache/DiskCacheManager");
var _constants = _interopRequireDefault(require("./constants"));
var _checkWatchmanCapabilities = _interopRequireDefault(
  require("./lib/checkWatchmanCapabilities"),
);
var _FileProcessor = require("./lib/FileProcessor");
var _normalizePathSeparatorsToPosix = _interopRequireDefault(
  require("./lib/normalizePathSeparatorsToPosix"),
);
var _normalizePathSeparatorsToSystem = _interopRequireDefault(
  require("./lib/normalizePathSeparatorsToSystem"),
);
var _RootPathUtils = require("./lib/RootPathUtils");
var _TreeFS = _interopRequireDefault(require("./lib/TreeFS"));
var _HastePlugin = _interopRequireDefault(require("./plugins/HastePlugin"));
var _MockPlugin = _interopRequireDefault(require("./plugins/MockPlugin"));
var _Watcher = require("./Watcher");
var _events = _interopRequireDefault(require("events"));
var _fs = require("fs");
var _invariant = _interopRequireDefault(require("invariant"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
var path = _interopRequireWildcard(require("path"));
var _perf_hooks = require("perf_hooks");
var _DuplicateHasteCandidatesError = require("./plugins/haste/DuplicateHasteCandidatesError");
var _HasteConflictsError = require("./plugins/haste/HasteConflictsError");
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
const debug = require("debug")("Metro:FileMap");
const CACHE_BREAKER = "10";
const CHANGE_INTERVAL = 30;
const NODE_MODULES = path.sep + "node_modules" + path.sep;
const PACKAGE_JSON = path.sep + "package.json";
const VCS_DIRECTORIES = /[/\\]\.(git|hg)[/\\]/.source;
const WATCHMAN_REQUIRED_CAPABILITIES = [
  "field-content.sha1hex",
  "relative_root",
  "suffix-set",
  "wildmatch",
];
class FileMap extends _events.default {
  #hastePlugin;
  #mockPlugin = null;
  #plugins;
  static create(options) {
    return new FileMap(options);
  }
  constructor(options) {
    super();
    if (options.perfLoggerFactory) {
      this._startupPerfLogger =
        options.perfLoggerFactory?.("START_UP").subSpan("fileMap") ?? null;
      this._startupPerfLogger?.point("constructor_start");
    }
    let ignorePattern;
    if (options.ignorePattern) {
      const inputIgnorePattern = options.ignorePattern;
      if (inputIgnorePattern instanceof RegExp) {
        ignorePattern = new RegExp(
          inputIgnorePattern.source.concat("|" + VCS_DIRECTORIES),
          inputIgnorePattern.flags,
        );
      } else {
        throw new Error(
          "metro-file-map: the `ignorePattern` option must be a RegExp",
        );
      }
    } else {
      ignorePattern = new RegExp(VCS_DIRECTORIES);
    }
    this._console = options.console || global.console;
    const throwOnModuleCollision = Boolean(options.throwOnModuleCollision);
    const enableHastePackages = options.enableHastePackages ?? true;
    this.#hastePlugin = new _HastePlugin.default({
      console: this._console,
      enableHastePackages,
      perfLogger: this._startupPerfLogger,
      platforms: new Set(options.platforms),
      rootDir: options.rootDir,
      failValidationOnConflicts: throwOnModuleCollision,
    });
    const plugins = [this.#hastePlugin];
    if (options.mocksPattern != null && options.mocksPattern !== "") {
      this.#mockPlugin = new _MockPlugin.default({
        console: this._console,
        mocksPattern: new RegExp(options.mocksPattern),
        rootDir: options.rootDir,
        throwOnModuleCollision,
      });
      plugins.push(this.#mockPlugin);
    }
    this.#plugins = plugins;
    const buildParameters = {
      computeDependencies:
        options.computeDependencies == null
          ? true
          : options.computeDependencies,
      computeSha1: options.computeSha1 || false,
      dependencyExtractor: options.dependencyExtractor ?? null,
      enableHastePackages,
      enableSymlinks: options.enableSymlinks || false,
      extensions: options.extensions,
      forceNodeFilesystemAPI: !!options.forceNodeFilesystemAPI,
      hasteImplModulePath: options.hasteImplModulePath,
      ignorePattern,
      plugins: options.plugins ?? [],
      retainAllFiles: options.retainAllFiles,
      rootDir: options.rootDir,
      roots: Array.from(new Set(options.roots)),
      skipPackageJson: !!options.skipPackageJson,
      cacheBreaker: CACHE_BREAKER,
    };
    this._options = {
      ...buildParameters,
      healthCheck: options.healthCheck,
      perfLoggerFactory: options.perfLoggerFactory,
      resetCache: options.resetCache,
      useWatchman: options.useWatchman == null ? true : options.useWatchman,
      watch: !!options.watch,
      watchmanDeferStates: options.watchmanDeferStates ?? [],
    };
    const cacheFactoryOptions = {
      buildParameters,
    };
    this._cacheManager = options.cacheManagerFactory
      ? options.cacheManagerFactory.call(null, cacheFactoryOptions)
      : new _DiskCacheManager.DiskCacheManager(cacheFactoryOptions, {});
    this._fileProcessor = new _FileProcessor.FileProcessor({
      dependencyExtractor: buildParameters.dependencyExtractor,
      enableHastePackages: buildParameters.enableHastePackages,
      enableWorkerThreads: options.enableWorkerThreads ?? false,
      hasteImplModulePath: buildParameters.hasteImplModulePath,
      maxFilesPerWorker: options.maxFilesPerWorker,
      maxWorkers: options.maxWorkers,
      perfLogger: this._startupPerfLogger,
    });
    this._buildPromise = null;
    this._pathUtils = new _RootPathUtils.RootPathUtils(options.rootDir);
    this._startupPerfLogger?.point("constructor_end");
    this._crawlerAbortController = new AbortController();
    this._changeID = 0;
  }
  build() {
    this._startupPerfLogger?.point("build_start");
    if (!this._buildPromise) {
      this._buildPromise = (async () => {
        let initialData;
        if (this._options.resetCache !== true) {
          initialData = await this.read();
        }
        if (!initialData) {
          debug("Not using a cache");
        } else {
          debug("Cache loaded (%d clock(s))", initialData.clocks.size);
        }
        const rootDir = this._options.rootDir;
        this._startupPerfLogger?.point("constructFileSystem_start");
        const processFile = (absolutePath, metadata, opts) => {
          const result = this._fileProcessor.processRegularFile(
            absolutePath,
            metadata,
            {
              computeSha1: opts.computeSha1,
              computeDependencies: false,
              maybeReturnContent: true,
            },
          );
          debug("Lazily processed file: %s", absolutePath);
          this.emit("metadata");
          return result?.content;
        };
        const fileSystem =
          initialData != null
            ? _TreeFS.default.fromDeserializedSnapshot({
                rootDir,
                fileSystemData: initialData.fileSystemData,
                processFile,
              })
            : new _TreeFS.default({
                rootDir,
                processFile,
              });
        this._startupPerfLogger?.point("constructFileSystem_end");
        const plugins = this.#plugins;
        const [fileDelta] = await Promise.all([
          this._buildFileDelta({
            fileSystem,
            clocks: initialData?.clocks ?? new Map(),
          }),
          Promise.all(
            plugins.map((plugin) =>
              plugin.initialize({
                files: fileSystem,
                pluginState: initialData?.plugins.get(plugin.name),
              }),
            ),
          ),
        ]);
        await this._applyFileDelta(fileSystem, plugins, fileDelta);
        plugins.forEach((plugin) => plugin.assertValid());
        const watchmanClocks = new Map(fileDelta.clocks ?? []);
        await this._takeSnapshotAndPersist(
          fileSystem,
          watchmanClocks,
          plugins,
          fileDelta.changedFiles,
          fileDelta.removedFiles,
        );
        debug(
          "Finished mapping files (%d changes, %d removed).",
          fileDelta.changedFiles.size,
          fileDelta.removedFiles.size,
        );
        await this._watch(fileSystem, watchmanClocks, plugins);
        return {
          fileSystem,
          hasteMap: this.#hastePlugin,
          mockMap: this.#mockPlugin,
        };
      })();
    }
    return this._buildPromise.then((result) => {
      this._startupPerfLogger?.point("build_end");
      return result;
    });
  }
  async read() {
    let data;
    this._startupPerfLogger?.point("read_start");
    try {
      data = await this._cacheManager.read();
    } catch (e) {
      this._console.warn(
        "Error while reading cache, falling back to a full crawl:\n",
        e,
      );
      this._startupPerfLogger?.annotate({
        string: {
          cacheReadError: e.toString(),
        },
      });
    }
    this._startupPerfLogger?.point("read_end");
    return data;
  }
  async _buildFileDelta(previousState) {
    this._startupPerfLogger?.point("buildFileDelta_start");
    const {
      computeSha1,
      enableSymlinks,
      extensions,
      forceNodeFilesystemAPI,
      ignorePattern,
      retainAllFiles,
      roots,
      rootDir,
      watch,
      watchmanDeferStates,
    } = this._options;
    this._watcher = new _Watcher.Watcher({
      abortSignal: this._crawlerAbortController.signal,
      computeSha1,
      console: this._console,
      enableSymlinks,
      extensions,
      forceNodeFilesystemAPI,
      healthCheckFilePrefix: this._options.healthCheck.filePrefix,
      ignoreForCrawl: (filePath) => {
        const ignoreMatched = ignorePattern.test(filePath);
        return (
          ignoreMatched || (!retainAllFiles && filePath.includes(NODE_MODULES))
        );
      },
      ignorePatternForWatch: ignorePattern,
      perfLogger: this._startupPerfLogger,
      previousState,
      roots,
      rootDir,
      useWatchman: await this._shouldUseWatchman(),
      watch,
      watchmanDeferStates,
    });
    const watcher = this._watcher;
    watcher.on("status", (status) => this.emit("status", status));
    return watcher.crawl().then((result) => {
      this._startupPerfLogger?.point("buildFileDelta_end");
      return result;
    });
  }
  _maybeReadLink(filePath, fileMetadata) {
    if (fileMetadata[_constants.default.SYMLINK] === 1) {
      return _fs.promises.readlink(filePath).then((symlinkTarget) => {
        fileMetadata[_constants.default.VISITED] = 1;
        fileMetadata[_constants.default.SYMLINK] = symlinkTarget;
      });
    }
    return null;
  }
  async _applyFileDelta(fileSystem, plugins, delta) {
    this._startupPerfLogger?.point("applyFileDelta_start");
    const { changedFiles, removedFiles } = delta;
    this._startupPerfLogger?.point("applyFileDelta_preprocess_start");
    const missingFiles = new Set();
    this._startupPerfLogger?.point("applyFileDelta_remove_start");
    const removed = [];
    for (const relativeFilePath of removedFiles) {
      const metadata = fileSystem.remove(relativeFilePath);
      if (metadata) {
        removed.push([relativeFilePath, metadata]);
      }
    }
    this._startupPerfLogger?.point("applyFileDelta_remove_end");
    const readLinkPromises = [];
    const readLinkErrors = [];
    const filesToProcess = [];
    for (const [relativeFilePath, fileData] of changedFiles) {
      if (fileData[_constants.default.VISITED] === 1) {
        continue;
      }
      if (
        this._options.skipPackageJson &&
        relativeFilePath.endsWith(PACKAGE_JSON)
      ) {
        continue;
      }
      if (
        fileData[_constants.default.SYMLINK] === 0 &&
        !this._options.computeDependencies &&
        !this._options.computeSha1 &&
        this._options.hasteImplModulePath == null &&
        !(
          this._options.enableHastePackages &&
          relativeFilePath.endsWith(PACKAGE_JSON)
        )
      ) {
        continue;
      }
      const absolutePath = this._pathUtils.normalToAbsolute(relativeFilePath);
      if (fileData[_constants.default.SYMLINK] === 0) {
        filesToProcess.push([absolutePath, fileData]);
      } else {
        const maybeReadLink = this._maybeReadLink(absolutePath, fileData);
        if (maybeReadLink) {
          readLinkPromises.push(
            maybeReadLink.catch((error) =>
              readLinkErrors.push({
                absolutePath,
                error,
              }),
            ),
          );
        }
      }
    }
    this._startupPerfLogger?.point("applyFileDelta_preprocess_end");
    debug(
      "Visiting %d added/modified files and %d symlinks.",
      filesToProcess.length,
      readLinkPromises.length,
    );
    this._startupPerfLogger?.point("applyFileDelta_process_start");
    const [batchResult] = await Promise.all([
      this._fileProcessor.processBatch(filesToProcess, {
        computeSha1: this._options.computeSha1,
        computeDependencies: this._options.computeDependencies,
        maybeReturnContent: false,
      }),
      Promise.all(readLinkPromises),
    ]);
    this._startupPerfLogger?.point("applyFileDelta_process_end");
    this._startupPerfLogger?.point("applyFileDelta_missing_start");
    for (const { absolutePath, error } of batchResult.errors.concat(
      readLinkErrors,
    )) {
      if (["ENOENT", "EACCESS"].includes(error.code)) {
        missingFiles.add(this._pathUtils.absoluteToNormal(absolutePath));
      } else {
        throw error;
      }
    }
    for (const relativeFilePath of missingFiles) {
      changedFiles.delete(relativeFilePath);
      const metadata = fileSystem.remove(relativeFilePath);
      if (metadata) {
        removed.push([relativeFilePath, metadata]);
      }
    }
    this._startupPerfLogger?.point("applyFileDelta_missing_end");
    this._startupPerfLogger?.point("applyFileDelta_add_start");
    fileSystem.bulkAddOrModify(changedFiles);
    this._startupPerfLogger?.point("applyFileDelta_add_end");
    this._startupPerfLogger?.point("applyFileDelta_updatePlugins_start");
    await Promise.all([
      plugins.map((plugin) =>
        plugin.bulkUpdate({
          addedOrModified: changedFiles,
          removed,
        }),
      ),
    ]);
    this._startupPerfLogger?.point("applyFileDelta_updatePlugins_end");
    this._startupPerfLogger?.point("applyFileDelta_end");
  }
  async _takeSnapshotAndPersist(fileSystem, clocks, plugins, changed, removed) {
    this._startupPerfLogger?.point("persist_start");
    await this._cacheManager.write(
      () => ({
        fileSystemData: fileSystem.getSerializableSnapshot(),
        clocks: new Map(clocks),
        plugins: new Map(
          plugins.map((plugin) => [
            plugin.name,
            plugin.getSerializableSnapshot(),
          ]),
        ),
      }),
      {
        changedSinceCacheRead: changed.size + removed.size > 0,
        eventSource: {
          onChange: (cb) => {
            this.on("change", cb);
            this.on("metadata", cb);
            return () => {
              this.removeListener("change", cb);
              this.removeListener("metadata", cb);
            };
          },
        },
        onWriteError: (error) => {
          this._console.warn("[metro-file-map] Cache write error\n:", error);
        },
      },
    );
    this._startupPerfLogger?.point("persist_end");
  }
  async _watch(fileSystem, clocks, plugins) {
    this._startupPerfLogger?.point("watch_start");
    if (!this._options.watch) {
      this._startupPerfLogger?.point("watch_end");
      return;
    }
    const hasWatchedExtension = (filePath) =>
      this._options.extensions.some((ext) => filePath.endsWith(ext));
    let changeQueue = Promise.resolve();
    let nextEmit = null;
    const emitChange = () => {
      if (nextEmit == null || nextEmit.eventsQueue.length === 0) {
        return;
      }
      const { eventsQueue, firstEventTimestamp, firstEnqueuedTimestamp } =
        nextEmit;
      const hmrPerfLogger = this._options.perfLoggerFactory?.("HMR", {
        key: this._getNextChangeID(),
      });
      if (hmrPerfLogger != null) {
        hmrPerfLogger.start({
          timestamp: firstEventTimestamp,
        });
        hmrPerfLogger.point("waitingForChangeInterval_start", {
          timestamp: firstEnqueuedTimestamp,
        });
        hmrPerfLogger.point("waitingForChangeInterval_end");
        hmrPerfLogger.annotate({
          int: {
            eventsQueueLength: eventsQueue.length,
          },
        });
        hmrPerfLogger.point("fileChange_start");
      }
      const changeEvent = {
        logger: hmrPerfLogger,
        eventsQueue,
      };
      this.emit("change", changeEvent);
      nextEmit = null;
    };
    const onChange = (change) => {
      if (
        change.metadata &&
        (change.metadata.type === "d" ||
          (change.metadata.type === "f" &&
            !hasWatchedExtension(change.relativePath)) ||
          (!this._options.enableSymlinks && change.metadata?.type === "l"))
      ) {
        return;
      }
      const absoluteFilePath = path.join(
        change.root,
        (0, _normalizePathSeparatorsToSystem.default)(change.relativePath),
      );
      if (this._options.ignorePattern.test(absoluteFilePath)) {
        return;
      }
      const relativeFilePath =
        this._pathUtils.absoluteToNormal(absoluteFilePath);
      const linkStats = fileSystem.linkStats(relativeFilePath);
      if (
        change.event === "touch" &&
        linkStats != null &&
        change.metadata.modifiedTime != null &&
        linkStats.modifiedTime === change.metadata.modifiedTime
      ) {
        return;
      }
      const eventTypeToEmit =
        change.event === "touch"
          ? linkStats == null
            ? "add"
            : "change"
          : "delete";
      const onChangeStartTime =
        _perf_hooks.performance.timeOrigin + _perf_hooks.performance.now();
      changeQueue = changeQueue
        .then(async () => {
          if (
            nextEmit != null &&
            nextEmit.eventsQueue.find(
              (event) =>
                event.type === eventTypeToEmit &&
                event.filePath === absoluteFilePath &&
                ((!event.metadata && !change.metadata) ||
                  (event.metadata &&
                    change.metadata &&
                    event.metadata.modifiedTime != null &&
                    change.metadata.modifiedTime != null &&
                    event.metadata.modifiedTime ===
                      change.metadata.modifiedTime)),
            )
          ) {
            return null;
          }
          const linkStats = fileSystem.linkStats(relativeFilePath);
          const enqueueEvent = (metadata) => {
            const event = {
              filePath: absoluteFilePath,
              metadata,
              type: eventTypeToEmit,
            };
            if (nextEmit == null) {
              nextEmit = {
                eventsQueue: [event],
                firstEventTimestamp: onChangeStartTime,
                firstEnqueuedTimestamp:
                  _perf_hooks.performance.timeOrigin +
                  _perf_hooks.performance.now(),
              };
            } else {
              nextEmit.eventsQueue.push(event);
            }
            return null;
          };
          if (change.event === "touch") {
            (0, _invariant.default)(
              change.metadata.size != null,
              "since the file exists or changed, it should have known size",
            );
            const fileMetadata = [
              change.metadata.modifiedTime,
              change.metadata.size,
              0,
              "",
              null,
              change.metadata.type === "l" ? 1 : 0,
              "",
            ];
            try {
              if (change.metadata.type === "l") {
                await this._maybeReadLink(absoluteFilePath, fileMetadata);
              } else {
                await this._fileProcessor.processRegularFile(
                  absoluteFilePath,
                  fileMetadata,
                  {
                    computeSha1: this._options.computeSha1,
                    computeDependencies: this._options.computeDependencies,
                    maybeReturnContent: false,
                  },
                );
              }
              fileSystem.addOrModify(relativeFilePath, fileMetadata);
              this._updateClock(clocks, change.clock);
              plugins.forEach((plugin) =>
                plugin.onNewOrModifiedFile(relativeFilePath, fileMetadata),
              );
              enqueueEvent(change.metadata);
            } catch (e) {
              if (!["ENOENT", "EACCESS"].includes(e.code)) {
                throw e;
              }
            }
          } else if (change.event === "delete") {
            if (linkStats == null) {
              return null;
            }
            const metadata = (0, _nullthrows.default)(
              fileSystem.remove(relativeFilePath),
            );
            this._updateClock(clocks, change.clock);
            plugins.forEach((plugin) =>
              plugin.onRemovedFile(relativeFilePath, metadata),
            );
            enqueueEvent({
              modifiedTime: null,
              size: null,
              type: linkStats.fileType,
            });
          } else {
            throw new Error(
              `metro-file-map: Unrecognized event type from watcher: ${change.event}`,
            );
          }
          return null;
        })
        .catch((error) => {
          this._console.error(
            `metro-file-map: watch error:\n  ${error.stack}\n`,
          );
        });
    };
    this._changeInterval = setInterval(emitChange, CHANGE_INTERVAL);
    (0, _invariant.default)(
      this._watcher != null,
      "Expected _watcher to have been initialised by build()",
    );
    await this._watcher.watch(onChange);
    if (this._options.healthCheck.enabled) {
      const performHealthCheck = () => {
        if (!this._watcher) {
          return;
        }
        this._watcher
          .checkHealth(this._options.healthCheck.timeout)
          .then((result) => {
            this.emit("healthCheck", result);
          });
      };
      performHealthCheck();
      this._healthCheckInterval = setInterval(
        performHealthCheck,
        this._options.healthCheck.interval,
      );
    }
    this._startupPerfLogger?.point("watch_end");
  }
  async end() {
    if (this._changeInterval) {
      clearInterval(this._changeInterval);
    }
    if (this._healthCheckInterval) {
      clearInterval(this._healthCheckInterval);
    }
    this._crawlerAbortController.abort();
    await Promise.all([
      this._fileProcessor.end(),
      this._watcher?.close(),
      this._cacheManager.end(),
    ]);
  }
  async _shouldUseWatchman() {
    if (!this._options.useWatchman) {
      return false;
    }
    if (!this._canUseWatchmanPromise) {
      this._canUseWatchmanPromise = (0, _checkWatchmanCapabilities.default)(
        WATCHMAN_REQUIRED_CAPABILITIES,
      )
        .then(({ version }) => {
          this._startupPerfLogger?.annotate({
            string: {
              watchmanVersion: version,
            },
          });
          return true;
        })
        .catch((e) => {
          this._startupPerfLogger?.annotate({
            string: {
              watchmanFailedCapabilityCheck: e?.message ?? "[missing]",
            },
          });
          return false;
        });
    }
    return this._canUseWatchmanPromise;
  }
  _getNextChangeID() {
    if (this._changeID >= Number.MAX_SAFE_INTEGER) {
      this._changeID = 0;
    }
    return ++this._changeID;
  }
  _updateClock(clocks, newClock) {
    if (newClock == null) {
      return;
    }
    const [absoluteWatchRoot, clockSpec] = newClock;
    const relativeFsRoot = this._pathUtils.absoluteToNormal(absoluteWatchRoot);
    clocks.set(
      (0, _normalizePathSeparatorsToPosix.default)(relativeFsRoot),
      clockSpec,
    );
  }
  static H = _constants.default;
}
exports.default = FileMap;
