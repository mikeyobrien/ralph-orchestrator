"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = createDefaultContext;
var _PackageResolve = require("./PackageResolve");
function createDefaultContext(context, dependency) {
  return {
    redirectModulePath: (modulePath) =>
      (0, _PackageResolve.redirectModulePath)(context, modulePath),
    dependency,
    ...context,
  };
}
