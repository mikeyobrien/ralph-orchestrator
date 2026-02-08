"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = getAllFiles;
var _Assets = require("../../Assets");
var _js = require("./helpers/js");
async function getAllFiles(pre, graph, options) {
  const modules = graph.dependencies;
  const { processModuleFilter } = options;
  const promises = [];
  for (const module of pre) {
    if (processModuleFilter(module)) {
      promises.push([module.path]);
    }
  }
  for (const module of modules.values()) {
    if (!(0, _js.isJsModule)(module) || !processModuleFilter(module)) {
      continue;
    }
    if ((0, _js.getJsOutput)(module).type === "js/module/asset") {
      promises.push((0, _Assets.getAssetFiles)(module.path, options.platform));
    } else {
      promises.push([module.path]);
    }
  }
  const dependencies = await Promise.all(promises);
  const output = [];
  for (const dependencyArray of dependencies) {
    output.push(...dependencyArray);
  }
  return output;
}
