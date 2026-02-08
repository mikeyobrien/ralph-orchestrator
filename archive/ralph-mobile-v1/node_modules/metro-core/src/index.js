"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
Object.defineProperty(exports, "AmbiguousModuleResolutionError", {
  enumerable: true,
  get: function () {
    return _AmbiguousModuleResolutionError.default;
  },
});
exports.Logger = void 0;
Object.defineProperty(exports, "PackageResolutionError", {
  enumerable: true,
  get: function () {
    return _PackageResolutionError.default;
  },
});
Object.defineProperty(exports, "Terminal", {
  enumerable: true,
  get: function () {
    return _Terminal.default;
  },
});
exports.default = void 0;
var _AmbiguousModuleResolutionError = _interopRequireDefault(
  require("./errors/AmbiguousModuleResolutionError"),
);
var _PackageResolutionError = _interopRequireDefault(
  require("./errors/PackageResolutionError"),
);
var Logger = _interopRequireWildcard(require("./Logger"));
exports.Logger = Logger;
var _Terminal = _interopRequireDefault(require("./Terminal"));
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
var _default = (exports.default = {
  AmbiguousModuleResolutionError: _AmbiguousModuleResolutionError.default,
  Logger,
  PackageResolutionError: _PackageResolutionError.default,
  Terminal: _Terminal.default,
});
