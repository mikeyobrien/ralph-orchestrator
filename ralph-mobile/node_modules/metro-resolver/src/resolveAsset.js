"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = resolveAsset;
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function resolveAsset(context, filePath) {
  const dirPath = _path.default.dirname(filePath);
  const extension = _path.default.extname(filePath);
  const basename = _path.default.basename(filePath, extension);
  try {
    if (!/@\d+(?:\.\d+)?x$/.test(basename)) {
      const assets = context.resolveAsset(dirPath, basename, extension);
      if (assets != null) {
        return {
          type: "assetFiles",
          filePaths: assets,
        };
      }
    }
  } catch (e) {}
  return null;
}
