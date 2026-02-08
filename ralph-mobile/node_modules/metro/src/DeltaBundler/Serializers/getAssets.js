"use strict";

const { getAssetData } = require("../../Assets");
const { getJsOutput, isJsModule } = require("./helpers/js");
const path = require("path");
async function getAssets(dependencies, options) {
  const promises = [];
  const { processModuleFilter } = options;
  for (const module of dependencies.values()) {
    if (
      isJsModule(module) &&
      processModuleFilter(module) &&
      getJsOutput(module).type === "js/module/asset" &&
      path.relative(options.projectRoot, module.path) !== "package.json"
    ) {
      promises.push(
        getAssetData(
          module.path,
          path.relative(options.projectRoot, module.path),
          options.assetPlugins,
          options.platform,
          options.publicPath
        )
      );
    }
  }
  return await Promise.all(promises);
}
module.exports = getAssets;
