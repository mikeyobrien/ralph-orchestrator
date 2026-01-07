"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = toPosixPath;
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const MATCH_NON_POSIX_PATH_SEPS = new RegExp(
  "\\" + _path.default.win32.sep,
  "g"
);
function toPosixPath(relativePathOrSpecifier) {
  if (_path.default.sep === _path.default.posix.sep) {
    return relativePathOrSpecifier;
  }
  return relativePathOrSpecifier.replace(
    MATCH_NON_POSIX_PATH_SEPS,
    _path.default.posix.sep
  );
}
