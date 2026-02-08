"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.getExplodedSourceMap = getExplodedSourceMap;
var _js = require("./helpers/js");
function getExplodedSourceMap(modules, options) {
  const modulesToProcess = modules
    .filter(_js.isJsModule)
    .filter(options.processModuleFilter);
  const result = [];
  let firstLine1Based = 1;
  for (const module of modulesToProcess) {
    const { path } = module;
    const { lineCount, functionMap, map } = (0, _js.getJsOutput)(module).data;
    result.push({
      firstLine1Based,
      functionMap,
      path,
      map,
    });
    firstLine1Based += lineCount;
  }
  return result;
}
