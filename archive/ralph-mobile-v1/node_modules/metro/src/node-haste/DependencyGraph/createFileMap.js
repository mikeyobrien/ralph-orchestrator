"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = createFileMap;
var _ciInfo = _interopRequireDefault(require("ci-info"));
var _metroFileMap = _interopRequireWildcard(require("metro-file-map"));
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
function getIgnorePattern(config) {
  const { blockList, blacklistRE } = config.resolver;
  const ignorePattern = blacklistRE || blockList;
  if (!ignorePattern) {
    return / ^/;
  }
  const combine = (regexes) =>
    new RegExp(
      regexes
        .map((regex, index) => {
          if (regex.flags !== regexes[0].flags) {
            throw new Error(
              "Cannot combine blockList patterns, because they have different flags:\n" +
                " - Pattern 0: " +
                regexes[0].toString() +
                "\n" +
                ` - Pattern ${index}: ` +
                regexes[index].toString(),
            );
          }
          return "(" + regex.source + ")";
        })
        .join("|"),
      regexes[0]?.flags ?? "",
    );
  if (Array.isArray(ignorePattern)) {
    return combine(ignorePattern);
  }
  return ignorePattern;
}
function createFileMap(config, options) {
  const dependencyExtractor =
    options?.extractDependencies === false
      ? null
      : config.resolver.dependencyExtractor;
  const computeDependencies = dependencyExtractor != null;
  const watch = options?.watch == null ? !_ciInfo.default.isCI : options.watch;
  const { enabled: autoSaveEnabled, ...autoSaveOpts } =
    config.watcher.unstable_autoSaveCache ?? {};
  const autoSave = watch && autoSaveEnabled ? autoSaveOpts : false;
  return _metroFileMap.default.create({
    cacheManagerFactory:
      config?.unstable_fileMapCacheManagerFactory ??
      ((factoryParams) =>
        new _metroFileMap.DiskCacheManager(factoryParams, {
          cacheDirectory:
            config.fileMapCacheDirectory ?? config.hasteMapCacheDirectory,
          cacheFilePrefix: options?.cacheFilePrefix,
          autoSave,
        })),
    perfLoggerFactory: config.unstable_perfLoggerFactory,
    computeDependencies,
    computeSha1: !config.watcher.unstable_lazySha1,
    dependencyExtractor: config.resolver.dependencyExtractor,
    enableHastePackages: config?.resolver.enableGlobalPackages,
    enableSymlinks: true,
    enableWorkerThreads: config.watcher.unstable_workerThreads,
    extensions: Array.from(
      new Set([
        ...config.resolver.sourceExts,
        ...config.resolver.assetExts,
        ...config.watcher.additionalExts,
      ]),
    ),
    forceNodeFilesystemAPI: !config.resolver.useWatchman,
    hasteImplModulePath: config.resolver.hasteImplModulePath,
    healthCheck: config.watcher.healthCheck,
    ignorePattern: getIgnorePattern(config),
    maxWorkers: config.maxWorkers,
    mocksPattern: "",
    platforms: [
      ...config.resolver.platforms,
      _metroFileMap.default.H.NATIVE_PLATFORM,
    ],
    retainAllFiles: true,
    resetCache: config.resetCache,
    rootDir: config.projectRoot,
    roots: config.watchFolders,
    throwOnModuleCollision: options?.throwOnModuleCollision ?? true,
    useWatchman: config.resolver.useWatchman,
    watch,
    watchmanDeferStates: config.watcher.watchman.deferStates,
  });
}
