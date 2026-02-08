"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _createFileMap = _interopRequireDefault(
  require("./DependencyGraph/createFileMap"),
);
var _ModuleResolution = require("./DependencyGraph/ModuleResolution");
var _PackageCache = require("./PackageCache");
var _events = _interopRequireDefault(require("events"));
var _fs = _interopRequireDefault(require("fs"));
var _metroCore = require("metro-core");
var _canonicalize = _interopRequireDefault(
  require("metro-core/private/canonicalize"),
);
var _metroFileMap = require("metro-file-map");
var _metroResolver = require("metro-resolver");
var _nullthrows = _interopRequireDefault(require("nullthrows"));
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const { createActionStartEntry, createActionEndEntry, log } = _metroCore.Logger;
const NULL_PLATFORM = Symbol();
function getOrCreateMap(map, field) {
  let subMap = map.get(field);
  if (!subMap) {
    subMap = new Map();
    map.set(field, subMap);
  }
  return subMap;
}
class DependencyGraph extends _events.default {
  #packageCache;
  constructor(config, options) {
    super();
    this._config = config;
    const { hasReducedPerformance, watch } = options ?? {};
    const initializingMetroLogEntry = log(
      createActionStartEntry("Initializing Metro"),
    );
    config.reporter.update({
      type: "dep_graph_loading",
      hasReducedPerformance: !!hasReducedPerformance,
    });
    const fileMap = (0, _createFileMap.default)(config, {
      throwOnModuleCollision: false,
      watch,
    });
    fileMap.setMaxListeners(1000);
    this._haste = fileMap;
    this._haste.on("status", (status) => this._onWatcherStatus(status));
    this._initializedPromise = fileMap
      .build()
      .then(({ fileSystem, hasteMap }) => {
        log(createActionEndEntry(initializingMetroLogEntry));
        config.reporter.update({
          type: "dep_graph_loaded",
        });
        this._fileSystem = fileSystem;
        this._hasteMap = hasteMap;
        this._haste.on("change", (changeEvent) =>
          this._onHasteChange(changeEvent),
        );
        this._haste.on("healthCheck", (result) =>
          this._onWatcherHealthCheck(result),
        );
        this._resolutionCache = new Map();
        this.#packageCache = this._createPackageCache();
        this._createModuleResolver();
      });
  }
  _onWatcherHealthCheck(result) {
    this._config.reporter.update({
      type: "watcher_health_check_result",
      result,
    });
  }
  _onWatcherStatus(status) {
    this._config.reporter.update({
      type: "watcher_status",
      status,
    });
  }
  async ready() {
    await this._initializedPromise;
  }
  static async load(config, options) {
    const self = new DependencyGraph(config, options);
    await self.ready();
    return self;
  }
  _onHasteChange({ eventsQueue }) {
    this._resolutionCache = new Map();
    eventsQueue.forEach(({ filePath }) =>
      this.#packageCache.invalidate(filePath),
    );
    this._createModuleResolver();
    this.emit("change");
  }
  _createModuleResolver() {
    const fileSystemLookup = (path) => {
      const result = this._fileSystem.lookup(path);
      if (result.exists) {
        return {
          exists: true,
          realPath: result.realPath,
          type: result.type,
        };
      }
      return {
        exists: false,
      };
    };
    this._moduleResolver = new _ModuleResolution.ModuleResolver({
      assetExts: new Set(this._config.resolver.assetExts),
      dirExists: (filePath) => {
        try {
          return _fs.default.lstatSync(filePath).isDirectory();
        } catch (e) {}
        return false;
      },
      disableHierarchicalLookup:
        this._config.resolver.disableHierarchicalLookup,
      doesFileExist: this._doesFileExist,
      emptyModulePath: this._config.resolver.emptyModulePath,
      extraNodeModules: this._config.resolver.extraNodeModules,
      fileSystemLookup,
      getHasteModulePath: (name, platform) =>
        this._hasteMap.getModule(name, platform, true),
      getHastePackagePath: (name, platform) =>
        this._hasteMap.getPackage(name, platform, true),
      mainFields: this._config.resolver.resolverMainFields,
      nodeModulesPaths: this._config.resolver.nodeModulesPaths,
      packageCache: this.#packageCache,
      preferNativePlatform: true,
      projectRoot: this._config.projectRoot,
      reporter: this._config.reporter,
      resolveAsset: (dirPath, assetName, extension) => {
        const basePath = dirPath + _path.default.sep + assetName;
        const assets = [
          basePath + extension,
          ...this._config.resolver.assetResolutions.map(
            (resolution) => basePath + "@" + resolution + "x" + extension,
          ),
        ]
          .map((assetPath) => fileSystemLookup(assetPath).realPath)
          .filter(Boolean);
        return assets.length ? assets : null;
      },
      resolveRequest: this._config.resolver.resolveRequest,
      sourceExts: this._config.resolver.sourceExts,
      unstable_conditionNames: this._config.resolver.unstable_conditionNames,
      unstable_conditionsByPlatform:
        this._config.resolver.unstable_conditionsByPlatform,
      unstable_enablePackageExports:
        this._config.resolver.unstable_enablePackageExports,
    });
  }
  _getClosestPackage(absoluteModulePath) {
    const result = this._fileSystem.hierarchicalLookup(
      absoluteModulePath,
      "package.json",
      {
        breakOnSegment: "node_modules",
        invalidatedBy: null,
        subpathType: "f",
      },
    );
    return result
      ? {
          packageJsonPath: result.absolutePath,
          packageRelativePath: result.containerRelativePath,
        }
      : null;
  }
  _createPackageCache() {
    return new _PackageCache.PackageCache({
      getClosestPackage: (absolutePath) =>
        this._getClosestPackage(absolutePath),
    });
  }
  getAllFiles() {
    return (0, _nullthrows.default)(this._fileSystem).getAllFiles();
  }
  async getOrComputeSha1(mixedPath) {
    const result = await this._fileSystem.getOrComputeSha1(mixedPath);
    if (!result || !result.sha1) {
      throw new Error(`Failed to get the SHA-1 for: ${mixedPath}.
      Potential causes:
        1) The file is not watched. Ensure it is under the configured \`projectRoot\` or \`watchFolders\`.
        2) Check \`blockList\` in your metro.config.js and make sure it isn't excluding the file path.
        3) The file may have been deleted since it was resolved - try refreshing your app.
        4) Otherwise, this is a bug in Metro or the configured resolver - please report it.`);
    }
    return result;
  }
  getWatcher() {
    return this._haste;
  }
  async end() {
    await this.ready();
    await this._haste.end();
  }
  matchFilesWithContext(from, context) {
    return this._fileSystem.matchFiles({
      rootDir: from,
      recursive: context.recursive,
      filter: context.filter,
      filterComparePosix: true,
      follow: true,
    });
  }
  resolveDependency(
    originModulePath,
    dependency,
    platform,
    resolverOptions,
    { assumeFlatNodeModules } = {
      assumeFlatNodeModules: false,
    },
  ) {
    const to = dependency.name;
    const isSensitiveToOriginFolder =
      !assumeFlatNodeModules ||
      to.includes("/") ||
      to === "." ||
      to === ".." ||
      originModulePath.includes(
        _path.default.sep + "node_modules" + _path.default.sep,
      );
    const resolverOptionsKey =
      JSON.stringify(resolverOptions ?? {}, _canonicalize.default) ?? "";
    const originKey = isSensitiveToOriginFolder
      ? _path.default.dirname(originModulePath)
      : "";
    const targetKey =
      to + (dependency.data.isESMImport === true ? "\0esm" : "\0cjs");
    const platformKey = platform ?? NULL_PLATFORM;
    const mapByResolverOptions = this._resolutionCache;
    const mapByOrigin = getOrCreateMap(
      mapByResolverOptions,
      resolverOptionsKey,
    );
    const mapByTarget = getOrCreateMap(mapByOrigin, originKey);
    const mapByPlatform = getOrCreateMap(mapByTarget, targetKey);
    let resolution = mapByPlatform.get(platformKey);
    if (!resolution) {
      try {
        resolution = this._moduleResolver.resolveDependency(
          originModulePath,
          dependency,
          true,
          platform,
          resolverOptions,
        );
      } catch (error) {
        if (error instanceof _metroFileMap.DuplicateHasteCandidatesError) {
          throw new _metroCore.AmbiguousModuleResolutionError(
            originModulePath,
            error,
          );
        }
        if (error instanceof _metroResolver.InvalidPackageError) {
          throw new _metroCore.PackageResolutionError({
            packageError: error,
            originModulePath,
            targetModuleName: to,
          });
        }
        throw error;
      }
    }
    mapByPlatform.set(platformKey, resolution);
    return resolution;
  }
  _doesFileExist = (filePath) => {
    return this._fileSystem.exists(filePath);
  };
  getHasteName(filePath) {
    const hasteName = this._fileSystem.getModuleName(filePath);
    if (hasteName) {
      return hasteName;
    }
    return _path.default.relative(this._config.projectRoot, filePath);
  }
  getDependencies(filePath) {
    return (0, _nullthrows.default)(this._fileSystem.getDependencies(filePath));
  }
}
exports.default = DependencyGraph;
