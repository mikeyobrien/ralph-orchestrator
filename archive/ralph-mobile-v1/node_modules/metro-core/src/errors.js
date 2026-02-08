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
Object.defineProperty(exports, "PackageResolutionError", {
  enumerable: true,
  get: function () {
    return _PackageResolutionError.default;
  },
});
var _AmbiguousModuleResolutionError = _interopRequireDefault(
  require("./errors/AmbiguousModuleResolutionError"),
);
var _PackageResolutionError = _interopRequireDefault(
  require("./errors/PackageResolutionError"),
);
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
