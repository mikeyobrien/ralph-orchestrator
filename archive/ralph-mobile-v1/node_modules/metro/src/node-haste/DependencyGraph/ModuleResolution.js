"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.UnableToResolveError = exports.ModuleResolver = void 0;
var _codeFrame = require("@babel/code-frame");
var _fs = _interopRequireDefault(require("fs"));
var _invariant = _interopRequireDefault(require("invariant"));
var Resolver = _interopRequireWildcard(require("metro-resolver"));
var _createDefaultContext = _interopRequireDefault(
  require("metro-resolver/private/createDefaultContext"),
);
var _path = _interopRequireDefault(require("path"));
var _util = _interopRequireDefault(require("util"));
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
class ModuleResolver {
  constructor(options) {
    this._options = options;
    const { projectRoot } = this._options;
    this._projectRootFakeModulePath = _path.default.join(projectRoot, "_");
  }
  _getEmptyModule() {
    let emptyModule = this._cachedEmptyModule;
    if (!emptyModule) {
      emptyModule = this.resolveDependency(
        this._projectRootFakeModulePath,
        {
          name: this._options.emptyModulePath,
          data: {
            key: this._options.emptyModulePath,
            asyncType: null,
            isESMImport: false,
            locs: [],
          },
        },
        false,
        null,
        {
          dev: false,
        },
      );
      this._cachedEmptyModule = emptyModule;
    }
    return emptyModule;
  }
  resolveDependency(
    originModulePath,
    dependency,
    allowHaste,
    platform,
    resolverOptions,
  ) {
    const {
      assetExts,
      disableHierarchicalLookup,
      doesFileExist,
      extraNodeModules,
      fileSystemLookup,
      mainFields,
      nodeModulesPaths,
      preferNativePlatform,
      resolveAsset,
      resolveRequest,
      sourceExts,
      unstable_conditionNames,
      unstable_conditionsByPlatform,
      unstable_enablePackageExports,
    } = this._options;
    try {
      const result = Resolver.resolve(
        (0, _createDefaultContext.default)(
          {
            allowHaste,
            assetExts,
            dev: resolverOptions.dev,
            disableHierarchicalLookup,
            doesFileExist,
            extraNodeModules,
            fileSystemLookup,
            isESMImport: dependency.data.isESMImport,
            mainFields,
            nodeModulesPaths,
            preferNativePlatform,
            resolveAsset,
            resolveRequest,
            sourceExts,
            unstable_conditionNames,
            unstable_conditionsByPlatform,
            unstable_enablePackageExports,
            unstable_logWarning: this._logWarning,
            customResolverOptions: resolverOptions.customResolverOptions ?? {},
            originModulePath,
            resolveHasteModule: (name) =>
              this._options.getHasteModulePath(name, platform),
            resolveHastePackage: (name) =>
              this._options.getHastePackagePath(name, platform),
            getPackage: this._getPackage,
            getPackageForModule: (absoluteModulePath) =>
              this._getPackageForModule(absoluteModulePath),
          },
          dependency,
        ),
        dependency.name,
        platform,
      );
      return this._getFileResolvedModule(result);
    } catch (error) {
      if (error instanceof Resolver.FailedToResolvePathError) {
        const { candidates } = error;
        throw new UnableToResolveError(
          originModulePath,
          dependency.name,
          "\n\nNone of these files exist:\n" +
            [candidates.file, candidates.dir]
              .filter(Boolean)
              .map(
                (candidates) =>
                  `  * ${Resolver.formatFileCandidates(this._removeRoot(candidates))}`,
              )
              .join("\n"),
          {
            cause: error,
            dependency,
          },
        );
      } else if (error instanceof Resolver.FailedToResolveUnsupportedError) {
        throw new UnableToResolveError(
          originModulePath,
          dependency.name,
          error.message,
          {
            cause: error,
            dependency,
          },
        );
      } else if (error instanceof Resolver.FailedToResolveNameError) {
        const dirPaths = error.dirPaths;
        const extraPaths = error.extraPaths;
        const displayDirPaths = dirPaths
          .filter((dirPath) => this._options.dirExists(dirPath))
          .map((dirPath) =>
            _path.default.relative(this._options.projectRoot, dirPath),
          )
          .concat(extraPaths);
        const hint = displayDirPaths.length ? " or in these directories:" : "";
        throw new UnableToResolveError(
          originModulePath,
          dependency.name,
          [
            `${dependency.name} could not be found within the project${hint || "."}`,
            ...displayDirPaths.map((dirPath) => `  ${dirPath}`),
          ].join("\n"),
          {
            cause: error,
            dependency,
          },
        );
      }
      throw error;
    }
  }
  _getPackage = (packageJsonPath) => {
    try {
      return this._options.packageCache.getPackage(packageJsonPath).read();
    } catch (e) {}
    return null;
  };
  _getPackageForModule = (absolutePath) => {
    let result;
    try {
      result = this._options.packageCache.getPackageOf(absolutePath);
    } catch (e) {}
    return result != null
      ? {
          rootPath: _path.default.dirname(result.pkg.path),
          packageJson: result.pkg.read(),
          packageRelativePath: result.packageRelativePath,
        }
      : null;
  };
  _getFileResolvedModule(resolution) {
    switch (resolution.type) {
      case "sourceFile":
        return resolution;
      case "assetFiles":
        const arbitrary = getArrayLowestItem(resolution.filePaths);
        (0, _invariant.default)(arbitrary != null, "invalid asset resolution");
        return {
          type: "sourceFile",
          filePath: arbitrary,
        };
      case "empty":
        return this._getEmptyModule();
      default:
        resolution.type;
        throw new Error("invalid type");
    }
  }
  _logWarning = (message) => {
    this._options.reporter.update({
      type: "resolver_warning",
      message,
    });
  };
  _removeRoot(candidates) {
    if (candidates.filePathPrefix) {
      candidates.filePathPrefix = _path.default.relative(
        this._options.projectRoot,
        candidates.filePathPrefix,
      );
    }
    return candidates;
  }
}
exports.ModuleResolver = ModuleResolver;
function getArrayLowestItem(a) {
  if (a.length === 0) {
    return undefined;
  }
  let lowest = a[0];
  for (let i = 1; i < a.length; ++i) {
    if (a[i] < lowest) {
      lowest = a[i];
    }
  }
  return lowest;
}
class UnableToResolveError extends Error {
  type = "UnableToResolveError";
  constructor(originModulePath, targetModuleName, message, options) {
    super();
    this.originModulePath = originModulePath;
    this.targetModuleName = targetModuleName;
    const codeFrameMessage = this.buildCodeFrameMessage(options?.dependency);
    this.message =
      _util.default.format(
        "Unable to resolve module %s from %s: %s",
        targetModuleName,
        originModulePath,
        message,
      ) + (codeFrameMessage ? "\n" + codeFrameMessage : "");
    this.cause = options?.cause;
  }
  buildCodeFrameMessage(dependency) {
    let file;
    try {
      file = _fs.default.readFileSync(this.originModulePath, "utf8");
    } catch (error) {
      if (error.code === "ENOENT" || error.code === "EISDIR") {
        return null;
      }
      throw error;
    }
    const location = dependency?.data.locs.length
      ? refineDependencyLocation(
          dependency.data.locs[0],
          file,
          this.targetModuleName,
        )
      : guessDependencyLocation(file, this.targetModuleName);
    return (0, _codeFrame.codeFrameColumns)(
      _fs.default.readFileSync(this.originModulePath, "utf8"),
      location,
      {
        forceColor: process.env.NODE_ENV !== "test",
      },
    );
  }
}
exports.UnableToResolveError = UnableToResolveError;
function refineDependencyLocation(loc, fileContents, targetSpecifier) {
  const lines = fileContents.split("\n");
  for (let line = loc.end.line - 1; line >= loc.start.line - 1; line--) {
    const maxColumn =
      line === loc.end.line ? loc.end.column + 2 : lines[line].length;
    const minColumn = line === loc.start.line ? loc.start.column - 1 : 0;
    const lineStr = lines[line];
    const lineSlice = lineStr.slice(minColumn, maxColumn);
    for (
      let offset = lineSlice.lastIndexOf(targetSpecifier);
      offset !== -1 && offset > 0 && offset < lineSlice.length - 1;
      offset = lineSlice.lastIndexOf(targetSpecifier, offset - 1)
    ) {
      const maybeQuoteBefore = lineSlice[minColumn + offset - 1];
      const maybeQuoteAfter =
        lineStr[minColumn + offset + targetSpecifier.length];
      if (isQuote(maybeQuoteBefore) && maybeQuoteBefore === maybeQuoteAfter) {
        return {
          start: {
            line: line + 1,
            column: minColumn + offset + 1,
          },
        };
      }
    }
  }
  if (loc.start.line === loc.end.line) {
    return {
      start: {
        line: loc.start.line,
        column: loc.start.column + 1,
      },
      end: {
        line: loc.end.line,
        column: loc.end.column + 1,
      },
    };
  }
  return {
    start: {
      line: loc.start.line,
      column: loc.start.column + 1,
    },
  };
}
function guessDependencyLocation(fileContents, targetSpecifier) {
  const lines = fileContents.split("\n");
  let lineNumber = 0;
  let column = -1;
  for (let line = 0; line < lines.length; line++) {
    const columnLocation = lines[line].lastIndexOf(targetSpecifier);
    if (columnLocation >= 0) {
      lineNumber = line;
      column = columnLocation;
      break;
    }
  }
  return {
    start: {
      column: column + 1,
      line: lineNumber + 1,
    },
  };
}
function isQuote(str) {
  return str === '"' || str === "'" || str === "`";
}
