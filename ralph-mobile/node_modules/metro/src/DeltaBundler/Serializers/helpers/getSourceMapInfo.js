"use strict";

const { getJsOutput } = require("./js");
function getSourceMapInfo(module, options) {
  return {
    ...getJsOutput(module).data,
    isIgnored: options.shouldAddToIgnoreList(module),
    path: options?.getSourceUrl?.(module) ?? module.path,
    source: options.excludeSource ? "" : getModuleSource(module),
  };
}
function getModuleSource(module) {
  if (getJsOutput(module).type === "js/module/asset") {
    return "";
  }
  return module.getSource().toString();
}
module.exports = getSourceMapInfo;
