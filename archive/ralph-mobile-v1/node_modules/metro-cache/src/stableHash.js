"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = stableHash;
var _crypto = _interopRequireDefault(require("crypto"));
var _canonicalize = _interopRequireDefault(
  require("metro-core/private/canonicalize"),
);
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function stableHash(value) {
  return _crypto.default
    .createHash("md5")
    .update(JSON.stringify(value, _canonicalize.default))
    .digest("buffer");
}
