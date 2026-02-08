"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.getCacheKey = getCacheKey;
var _crypto = _interopRequireDefault(require("crypto"));
var _fs = _interopRequireDefault(require("fs"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function getCacheKey(files) {
  return files
    .reduce(
      (hash, file) =>
        hash.update("\0", "utf8").update(_fs.default.readFileSync(file)),
      _crypto.default.createHash("md5"),
    )
    .digest("hex");
}
