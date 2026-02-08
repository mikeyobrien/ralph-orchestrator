"use strict";

const { getAssetFiles } = require("../../Assets");
const { getJsOutput, isJsModule } = require("./helpers/js");
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
    if (!isJsModule(module) || !processModuleFilter(module)) {
      continue;
    }
    if (getJsOutput(module).type === "js/module/asset") {
      promises.push(getAssetFiles(module.path, options.platform));
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
module.exports = getAllFiles;
