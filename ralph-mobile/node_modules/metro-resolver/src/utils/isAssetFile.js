"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = isAssetFile;
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function isAssetFile(filePath, assetExts) {
  const baseName = _path.default.basename(filePath);
  for (let i = baseName.length - 1; i >= 0; i--) {
    if (baseName[i] === ".") {
      const ext = baseName.slice(i + 1);
      if (assetExts.has(ext)) {
        return true;
      }
    }
  }
  return false;
}
