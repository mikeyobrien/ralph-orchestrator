"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = normalizeSourcePath;
var _util = _interopRequireDefault(require("source-map/lib/util"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function normalizeSourcePath(sourceInput, map) {
  const { sourceRoot } = map;
  let source = sourceInput;
  source = String(source);
  source = _util.default.normalize(source);
  source =
    sourceRoot != null &&
    _util.default.isAbsolute(sourceRoot) &&
    _util.default.isAbsolute(source)
      ? _util.default.relative(sourceRoot, source)
      : source;
  return source;
}
