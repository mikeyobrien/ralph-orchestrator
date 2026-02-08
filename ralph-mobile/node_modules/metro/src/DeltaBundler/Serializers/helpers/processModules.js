"use strict";

const { isJsModule, wrapModule } = require("./js");
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
  }
) {
  return [...modules]
    .filter(isJsModule)
    .filter(filter)
    .map((module) => [
      module,
      wrapModule(module, {
        createModuleId,
        dev,
        includeAsyncPaths,
        projectRoot,
        serverRoot,
        sourceUrl,
      }),
    ]);
}
module.exports = processModules;
