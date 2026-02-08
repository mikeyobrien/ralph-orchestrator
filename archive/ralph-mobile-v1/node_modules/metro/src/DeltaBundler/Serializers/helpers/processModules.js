"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = processModules;
var _js = require("./js");
function processModules(
  modules,
  {
    filter = () => true,
    createModuleId,
    dev,
    includeAsyncPaths,
    projectRoot,
    serverRoot,
    sourceUrl,
  },
) {
  return [...modules]
    .filter(_js.isJsModule)
    .filter(filter)
    .map((module) => [
      module,
      (0, _js.wrapModule)(module, {
        createModuleId,
        dev,
        includeAsyncPaths,
        projectRoot,
        serverRoot,
        sourceUrl,
      }),
    ]);
}
