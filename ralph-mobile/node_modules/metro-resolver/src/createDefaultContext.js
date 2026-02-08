"use strict";

var _PackageResolve = require("./PackageResolve");
function createDefaultContext(context, dependency) {
  return {
    redirectModulePath: (modulePath) =>
      (0, _PackageResolve.redirectModulePath)(context, modulePath),
    dependency,
    ...context,
  };
}
module.exports = createDefaultContext;
