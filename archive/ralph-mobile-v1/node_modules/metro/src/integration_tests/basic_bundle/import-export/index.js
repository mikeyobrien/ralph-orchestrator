"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.asyncImportMaybeSyncESM =
  exports.asyncImportMaybeSyncCJS =
  exports.asyncImportESM =
  exports.asyncImportCJS =
    void 0;
Object.defineProperty(exports, "default", {
  enumerable: true,
  get: function () {
    return _export4.foo;
  },
});
exports.extraData = void 0;
Object.defineProperty(exports, "namedDefaultExported", {
  enumerable: true,
  get: function () {
    return _export3.default;
  },
});
var _export = _interopRequireWildcard(require("./export-1"));
var importStar = _interopRequireWildcard(require("./export-2"));
var _exportNull = require("./export-null");
var _exportPrimitiveDefault = _interopRequireWildcard(
  require("./export-primitive-default"),
);
var _export3 = _interopRequireDefault(require("./export-3"));
var _export4 = require("./export-4");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
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
const extraData = (exports.extraData = {
  foo: _exportNull.foo,
  importStar,
  myDefault: _export.default,
  myFoo: _export.foo,
  myFunction: (0, _export.myFunction)(),
  primitiveDefault: _exportPrimitiveDefault.default,
  primitiveFoo: _exportPrimitiveDefault.foo,
});
const asyncImportCJS = (exports.asyncImportCJS = import("./export-5"));
const asyncImportESM = (exports.asyncImportESM = import("./export-6"));
const asyncImportMaybeSyncCJS = (exports.asyncImportMaybeSyncCJS =
  require.unstable_importMaybeSync("./export-7"));
const asyncImportMaybeSyncESM = (exports.asyncImportMaybeSyncESM =
  require.unstable_importMaybeSync("./export-8"));
