"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.resolvePackageTargetFromImports = resolvePackageTargetFromImports;
var _InvalidPackageConfigurationError = _interopRequireDefault(
  require("./errors/InvalidPackageConfigurationError")
);
var _PackageImportNotResolvedError = _interopRequireDefault(
  require("./errors/PackageImportNotResolvedError")
);
var _resolveAsset = _interopRequireDefault(require("./resolveAsset"));
var _isAssetFile = _interopRequireDefault(require("./utils/isAssetFile"));
var _isSubpathDefinedInExportsLike = require("./utils/isSubpathDefinedInExportsLike");
var _matchSubpathFromExportsLike = require("./utils/matchSubpathFromExportsLike");
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function resolvePackageTargetFromImports(
  context,
  packagePath,
  importPath,
  importsMap,
  platform
) {
  const createConfigError = (reason) => {
    return new _InvalidPackageConfigurationError.default({
      reason,
      packagePath,
    });
  };
  const firstLevelKeys = Object.keys(importsMap);
  const keysWithoutPrefix = firstLevelKeys.filter(
    (key) => !key.startsWith("#")
  );
  if (firstLevelKeys.length === 0) {
    throw createConfigError('The "imports" field cannot be empty');
  } else if (keysWithoutPrefix.length !== 0) {
    throw createConfigError(
      'The "imports" field cannot have keys which do not start with #'
    );
  }
  const normalizedMap = new Map(Object.entries(importsMap));
  if (
    !(0, _isSubpathDefinedInExportsLike.isSubpathDefinedInExportsLike)(
      normalizedMap,
      importPath
    )
  ) {
    throw new _PackageImportNotResolvedError.default({
      importSpecifier: importPath,
      reason: `"${importPath}" could not be matched using "imports" of ${packagePath}`,
    });
  }
  const { target, patternMatch } = (0,
  _matchSubpathFromExportsLike.matchSubpathFromExportsLike)(
    context,
    importPath,
    normalizedMap,
    platform,
    createConfigError
  );
  if (target == null) {
    throw new _PackageImportNotResolvedError.default({
      importSpecifier: importPath,
      reason:
        `"${importPath}" which matches a subpath "imports" in ${packagePath}` +
        `however no match was resolved for this request (platform = ${
          platform ?? "null"
        }).`,
    });
  }
  const filePath = _path.default.join(
    packagePath,
    patternMatch != null ? target.replaceAll("*", patternMatch) : target
  );
  if ((0, _isAssetFile.default)(filePath, context.assetExts)) {
    const assetResult = (0, _resolveAsset.default)(context, filePath);
    if (assetResult != null) {
      return assetResult;
    }
  }
  const lookupResult = context.fileSystemLookup(filePath);
  if (lookupResult.exists && lookupResult.type === "f") {
    return {
      type: "sourceFile",
      filePath: lookupResult.realPath,
    };
  }
  throw createConfigError(
    `The resolved path for "${importPath}" defined in "imports" is ${filePath}, ` +
      "however this file does not exist."
  );
}
