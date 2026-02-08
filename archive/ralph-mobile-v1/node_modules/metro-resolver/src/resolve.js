"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = resolve;
var _FailedToResolveNameError = _interopRequireDefault(
  require("./errors/FailedToResolveNameError"),
);
var _FailedToResolvePathError = _interopRequireDefault(
  require("./errors/FailedToResolvePathError"),
);
var _formatFileCandidates = _interopRequireDefault(
  require("./errors/formatFileCandidates"),
);
var _InvalidPackageConfigurationError = _interopRequireDefault(
  require("./errors/InvalidPackageConfigurationError"),
);
var _InvalidPackageError = _interopRequireDefault(
  require("./errors/InvalidPackageError"),
);
var _PackageImportNotResolvedError = _interopRequireDefault(
  require("./errors/PackageImportNotResolvedError"),
);
var _PackagePathNotExportedError = _interopRequireDefault(
  require("./errors/PackagePathNotExportedError"),
);
var _PackageExportsResolve = require("./PackageExportsResolve");
var _PackageImportsResolve = require("./PackageImportsResolve");
var _PackageResolve = require("./PackageResolve");
var _resolveAsset = _interopRequireDefault(require("./resolveAsset"));
var _isAssetFile = _interopRequireDefault(require("./utils/isAssetFile"));
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function resolve(context, moduleName, platform) {
  const resolveRequest = context.resolveRequest;
  if (resolveRequest && resolveRequest !== resolve) {
    return resolveRequest(
      Object.freeze({
        ...context,
        resolveRequest: resolve,
      }),
      moduleName,
      platform,
    );
  }
  if (isRelativeImport(moduleName) || _path.default.isAbsolute(moduleName)) {
    const result = resolveModulePath(context, moduleName, platform);
    if (result.type === "failed") {
      throw new _FailedToResolvePathError.default(result.candidates);
    }
    return result.resolution;
  } else if (isSubpathImport(moduleName)) {
    const pkg = context.getPackageForModule(context.originModulePath);
    const importsField = pkg?.packageJson.imports;
    if (pkg == null) {
      throw new _PackageImportNotResolvedError.default({
        importSpecifier: moduleName,
        reason: `Could not find a package.json file relative to module ${context.originModulePath}`,
      });
    } else if (importsField == null) {
      throw new _PackageImportNotResolvedError.default({
        importSpecifier: moduleName,
        reason: `Missing field "imports" in package.json. Check package.json at: ${pkg.rootPath}`,
      });
    } else {
      try {
        const packageImportsResult = (0,
        _PackageImportsResolve.resolvePackageTargetFromImports)(
          context,
          pkg.rootPath,
          moduleName,
          importsField,
          platform,
        );
        if (packageImportsResult != null) {
          return packageImportsResult;
        }
      } catch (e) {
        if (e instanceof _PackageImportNotResolvedError.default) {
          context.unstable_logWarning(
            e.message +
              " Falling back to file-based resolution. Consider updating the " +
              'call site or checking there is a matching subpath inside "imports" of package.json.',
          );
        } else if (e instanceof _InvalidPackageConfigurationError.default) {
          context.unstable_logWarning(
            e.message + " Falling back to file-based resolution.",
          );
        } else {
          throw e;
        }
      }
    }
  }
  const realModuleName = (0, _PackageResolve.redirectModulePath)(
    context,
    moduleName,
  );
  if (realModuleName === false) {
    return {
      type: "empty",
    };
  }
  const { originModulePath } = context;
  const isDirectImport =
    isRelativeImport(realModuleName) ||
    _path.default.isAbsolute(realModuleName);
  if (isDirectImport) {
    const fromModuleParentIdx =
      originModulePath.lastIndexOf("node_modules" + _path.default.sep) + 13;
    const originModuleDir = originModulePath.slice(
      0,
      originModulePath.indexOf(_path.default.sep, fromModuleParentIdx),
    );
    const absPath = _path.default.join(originModuleDir, realModuleName);
    const result = resolveModulePath(context, absPath, platform);
    if (result.type === "failed") {
      throw new _FailedToResolvePathError.default(result.candidates);
    }
    return result.resolution;
  }
  const parsedSpecifier = parseBareSpecifier(realModuleName);
  if (context.allowHaste) {
    if (parsedSpecifier.isSinglePart) {
      const result = context.resolveHasteModule(parsedSpecifier.firstPart);
      if (result != null) {
        return {
          type: "sourceFile",
          filePath: result,
        };
      }
    }
    if (parsedSpecifier.isValidPackageName) {
      const result = resolveHastePackage(context, parsedSpecifier, platform);
      if (result.type === "resolved") {
        return result.resolution;
      }
    }
  }
  const { disableHierarchicalLookup } = context;
  const nodeModulesPaths = [];
  let next = _path.default.dirname(originModulePath);
  if (!disableHierarchicalLookup) {
    let candidate;
    do {
      candidate = next;
      const nodeModulesPath = candidate.endsWith(_path.default.sep)
        ? candidate + "node_modules"
        : candidate + _path.default.sep + "node_modules";
      nodeModulesPaths.push(nodeModulesPath);
      next = _path.default.dirname(candidate);
    } while (candidate !== next);
  }
  nodeModulesPaths.push(...context.nodeModulesPaths);
  const extraPaths = [];
  const { extraNodeModules } = context;
  if (extraNodeModules && extraNodeModules[parsedSpecifier.packageName]) {
    const newPackageName = extraNodeModules[parsedSpecifier.packageName];
    extraPaths.push(
      _path.default.join(newPackageName, parsedSpecifier.posixSubpath),
    );
  }
  const allDirPaths = nodeModulesPaths
    .map((nodeModulePath) => {
      let lookupResult = null;
      const mustBeDirectory =
        parsedSpecifier.posixSubpath !== "." ||
        parsedSpecifier.packageName.length > parsedSpecifier.firstPart.length
          ? nodeModulePath + _path.default.sep + parsedSpecifier.firstPart
          : nodeModulePath;
      lookupResult = context.fileSystemLookup(mustBeDirectory);
      if (!lookupResult.exists || lookupResult.type !== "d") {
        return null;
      }
      return _path.default.join(nodeModulePath, realModuleName);
    })
    .filter(Boolean)
    .concat(extraPaths);
  for (let i = 0; i < allDirPaths.length; ++i) {
    const candidate = (0, _PackageResolve.redirectModulePath)(
      context,
      allDirPaths[i],
    );
    if (candidate === false) {
      return {
        type: "empty",
      };
    }
    const result = resolvePackage(context, candidate, platform);
    if (result.type === "resolved") {
      return result.resolution;
    }
  }
  throw new _FailedToResolveNameError.default(nodeModulesPaths, extraPaths);
}
function parseBareSpecifier(specifier) {
  const normalized =
    _path.default.sep === "/" ? specifier : specifier.replaceAll("\\", "/");
  const firstSepIdx = normalized.indexOf("/");
  if (normalized.startsWith("@") && firstSepIdx !== -1) {
    const secondSepIdx = normalized.indexOf("/", firstSepIdx + 1);
    if (secondSepIdx === -1) {
      return {
        isSinglePart: false,
        isValidPackageName: true,
        firstPart: normalized.slice(0, firstSepIdx),
        normalizedSpecifier: normalized,
        packageName: normalized,
        posixSubpath: ".",
      };
    }
    return {
      isSinglePart: false,
      isValidPackageName: true,
      firstPart: normalized.slice(0, firstSepIdx),
      normalizedSpecifier: normalized,
      packageName: normalized.slice(0, secondSepIdx),
      posixSubpath: "." + normalized.slice(secondSepIdx),
    };
  }
  if (firstSepIdx === -1) {
    return {
      isSinglePart: true,
      isValidPackageName: !normalized.startsWith("@"),
      firstPart: normalized,
      normalizedSpecifier: normalized,
      packageName: normalized,
      posixSubpath: ".",
    };
  }
  const packageName = normalized.slice(0, firstSepIdx);
  return {
    isSinglePart: false,
    isValidPackageName: true,
    firstPart: packageName,
    normalizedSpecifier: normalized,
    packageName,
    posixSubpath: "." + normalized.slice(firstSepIdx),
  };
}
function resolveModulePath(context, toModuleName, platform) {
  const modulePath = _path.default.isAbsolute(toModuleName)
    ? _path.default.sep === "/"
      ? toModuleName
      : toModuleName.replaceAll("/", "\\")
    : _path.default.join(
        _path.default.dirname(context.originModulePath),
        toModuleName,
      );
  const redirectedPath = (0, _PackageResolve.redirectModulePath)(
    context,
    modulePath,
  );
  if (redirectedPath === false) {
    return resolvedAs({
      type: "empty",
    });
  }
  const dirPath = _path.default.dirname(redirectedPath);
  const fileName = _path.default.basename(redirectedPath);
  const fileResult = redirectedPath.endsWith(_path.default.sep)
    ? null
    : resolveFile(context, dirPath, fileName, platform);
  if (fileResult != null && fileResult.type === "resolved") {
    return fileResult;
  }
  const dirResult = resolvePackageEntryPoint(context, redirectedPath, platform);
  if (dirResult.type === "resolved") {
    return dirResult;
  }
  return failedFor({
    file: fileResult?.candidates ?? null,
    dir: dirResult.candidates,
  });
}
function resolveHastePackage(
  context,
  { normalizedSpecifier: moduleName, packageName, posixSubpath: pathInModule },
  platform,
) {
  const packageJsonPath = context.resolveHastePackage(packageName);
  if (packageJsonPath == null) {
    return failedFor();
  }
  const potentialModulePath = _path.default.join(
    packageJsonPath,
    "..",
    pathInModule,
  );
  const result = resolvePackage(context, potentialModulePath, platform);
  if (result.type === "resolved") {
    return result;
  }
  const { candidates } = result;
  const opts = {
    moduleName,
    packageName,
    pathInModule,
    candidates,
  };
  throw new MissingFileInHastePackageError(opts);
}
class MissingFileInHastePackageError extends Error {
  constructor(opts) {
    super(
      `While resolving module \`${opts.moduleName}\`, ` +
        `the Haste package \`${opts.packageName}\` was found. However the ` +
        `subpath \`${opts.pathInModule}\` could not be found within ` +
        "the package. Indeed, none of these files exist:\n\n" +
        [opts.candidates.file, opts.candidates.dir]
          .filter(Boolean)
          .map(
            (candidates) =>
              `  * \`${(0, _formatFileCandidates.default)(candidates)}\``,
          )
          .join("\n"),
    );
    Object.assign(this, opts);
  }
}
function resolvePackage(context, absoluteCandidatePath, platform) {
  if (context.unstable_enablePackageExports) {
    const pkg = context.getPackageForModule(absoluteCandidatePath);
    const exportsField = pkg?.packageJson.exports;
    if (pkg != null && exportsField != null) {
      try {
        const packageExportsResult = (0,
        _PackageExportsResolve.resolvePackageTargetFromExports)(
          context,
          pkg.rootPath,
          absoluteCandidatePath,
          pkg.packageRelativePath,
          exportsField,
          platform,
        );
        if (packageExportsResult != null) {
          return resolvedAs(packageExportsResult);
        }
      } catch (e) {
        if (e instanceof _PackagePathNotExportedError.default) {
          context.unstable_logWarning(
            e.message +
              " Falling back to file-based resolution. Consider updating the " +
              "call site or asking the package maintainer(s) to expose this API.",
          );
        } else if (e instanceof _InvalidPackageConfigurationError.default) {
          context.unstable_logWarning(
            e.message + " Falling back to file-based resolution.",
          );
        } else {
          throw e;
        }
      }
    }
  }
  return resolveModulePath(context, absoluteCandidatePath, platform);
}
function resolvePackageEntryPoint(context, packagePath, platform) {
  const dirLookup = context.fileSystemLookup(packagePath);
  if (dirLookup.exists == false || dirLookup.type !== "d") {
    return failedFor({
      type: "sourceFile",
      filePathPrefix: packagePath,
      candidateExts: [],
    });
  }
  const packageJsonPath = _path.default.join(packagePath, "package.json");
  if (!context.doesFileExist(packageJsonPath)) {
    return resolveFile(context, packagePath, "index", platform);
  }
  const packageInfo = {
    rootPath: _path.default.dirname(packageJsonPath),
    packageJson: context.getPackage(packageJsonPath) ?? {},
  };
  const mainModulePath = _path.default.join(
    packageInfo.rootPath,
    (0, _PackageResolve.getPackageEntryPoint)(context, packageInfo, platform),
  );
  const fileResult = resolveFile(
    context,
    _path.default.dirname(mainModulePath),
    _path.default.basename(mainModulePath),
    platform,
  );
  if (fileResult.type === "resolved") {
    return fileResult;
  }
  const indexResult = resolveFile(context, mainModulePath, "index", platform);
  if (indexResult.type !== "resolved") {
    throw new _InvalidPackageError.default({
      packageJsonPath,
      mainModulePath,
      fileCandidates: fileResult.candidates,
      indexCandidates: indexResult.candidates,
    });
  }
  return indexResult;
}
function resolveFile(context, dirPath, fileName, platform) {
  if ((0, _isAssetFile.default)(fileName, context.assetExts)) {
    const assetResolutions = (0, _resolveAsset.default)(
      context,
      _path.default.join(dirPath, fileName),
    );
    if (assetResolutions == null) {
      return failedFor({
        type: "asset",
        name: fileName,
      });
    }
    return resolvedAs(assetResolutions);
  }
  const candidateExts = [];
  const filePathPrefix = _path.default.join(dirPath, fileName);
  const sfContext = {
    ...context,
    candidateExts,
    filePathPrefix,
  };
  const sourceFileResolution = resolveSourceFile(sfContext, platform);
  if (sourceFileResolution != null) {
    if (typeof sourceFileResolution === "string") {
      return resolvedAs({
        type: "sourceFile",
        filePath: sourceFileResolution,
      });
    }
    return resolvedAs(sourceFileResolution);
  }
  return failedFor({
    type: "sourceFile",
    filePathPrefix,
    candidateExts,
  });
}
function resolveSourceFile(context, platform) {
  let filePath = resolveSourceFileForAllExts(context, "");
  if (filePath) {
    return filePath;
  }
  const { sourceExts } = context;
  for (let i = 0; i < sourceExts.length; i++) {
    const ext = `.${sourceExts[i]}`;
    filePath = resolveSourceFileForAllExts(context, ext, platform);
    if (filePath != null) {
      return filePath;
    }
  }
  return null;
}
function resolveSourceFileForAllExts(context, sourceExt, platform) {
  if (platform != null) {
    const ext = `.${platform}${sourceExt}`;
    const filePath = resolveSourceFileForExt(context, ext);
    if (filePath) {
      return filePath;
    }
  }
  if (context.preferNativePlatform && sourceExt !== "") {
    const filePath = resolveSourceFileForExt(context, `.native${sourceExt}`);
    if (filePath) {
      return filePath;
    }
  }
  const filePath = resolveSourceFileForExt(context, sourceExt);
  return filePath;
}
function resolveSourceFileForExt(context, extension) {
  const filePath = `${context.filePathPrefix}${extension}`;
  const redirectedPath =
    extension !== ""
      ? (0, _PackageResolve.redirectModulePath)(context, filePath)
      : filePath;
  if (redirectedPath === false) {
    return {
      type: "empty",
    };
  }
  const lookupResult = context.fileSystemLookup(redirectedPath);
  if (lookupResult.exists && lookupResult.type === "f") {
    return lookupResult.realPath;
  }
  context.candidateExts.push(extension);
  return null;
}
function isRelativeImport(filePath) {
  return /^[.][.]?(?:[/]|$)/.test(filePath);
}
function isSubpathImport(filePath) {
  return filePath.startsWith("#");
}
function resolvedAs(resolution) {
  return {
    type: "resolved",
    resolution,
  };
}
function failedFor(candidates) {
  return {
    type: "failed",
    candidates,
  };
}
