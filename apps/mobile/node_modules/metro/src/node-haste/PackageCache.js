"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.PackageCache = void 0;
var _Package = _interopRequireDefault(require("./Package"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class PackageCache {
  constructor(options) {
    this._getClosestPackage = options.getClosestPackage;
    this._packageCache = Object.create(null);
    this._packagePathAndSubpathByModulePath = Object.create(null);
    this._modulePathsByPackagePath = Object.create(null);
  }
  getPackage(filePath) {
    if (!this._packageCache[filePath]) {
      this._packageCache[filePath] = new _Package.default({
        file: filePath,
      });
    }
    return this._packageCache[filePath];
  }
  getPackageOf(absoluteModulePath) {
    let packagePathAndSubpath =
      this._packagePathAndSubpathByModulePath[absoluteModulePath];
    if (
      packagePathAndSubpath &&
      this._packageCache[packagePathAndSubpath.packageJsonPath]
    ) {
      return {
        pkg: this._packageCache[packagePathAndSubpath.packageJsonPath],
        packageRelativePath: packagePathAndSubpath.packageRelativePath,
      };
    }
    packagePathAndSubpath = this._getClosestPackage(absoluteModulePath);
    if (!packagePathAndSubpath) {
      return null;
    }
    const packagePath = packagePathAndSubpath.packageJsonPath;
    this._packagePathAndSubpathByModulePath[absoluteModulePath] =
      packagePathAndSubpath;
    const modulePaths =
      this._modulePathsByPackagePath[packagePath] ?? new Set();
    modulePaths.add(absoluteModulePath);
    this._modulePathsByPackagePath[packagePath] = modulePaths;
    return {
      pkg: this.getPackage(packagePath),
      packageRelativePath: packagePathAndSubpath.packageRelativePath,
    };
  }
  invalidate(filePath) {
    if (this._packageCache[filePath]) {
      this._packageCache[filePath].invalidate();
      delete this._packageCache[filePath];
    }
    const packagePathAndSubpath =
      this._packagePathAndSubpathByModulePath[filePath];
    if (packagePathAndSubpath) {
      const packagePath = packagePathAndSubpath.packageJsonPath;
      delete this._packagePathAndSubpathByModulePath[filePath];
      const modulePaths = this._modulePathsByPackagePath[packagePath];
      if (modulePaths) {
        modulePaths.delete(filePath);
        if (modulePaths.size === 0) {
          delete this._modulePathsByPackagePath[packagePath];
        }
      }
    }
    if (this._modulePathsByPackagePath[filePath]) {
      const modulePaths = this._modulePathsByPackagePath[filePath];
      for (const modulePath of modulePaths) {
        delete this._packagePathAndSubpathByModulePath[modulePath];
      }
      modulePaths.clear();
      delete this._modulePathsByPackagePath[filePath];
    }
  }
}
exports.PackageCache = PackageCache;
