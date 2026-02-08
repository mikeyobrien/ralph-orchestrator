"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = getSourceMapInfo;
var _js = require("./js");
function getSourceMapInfo(module, options) {
  return {
    ...(0, _js.getJsOutput)(module).data,
    isIgnored: options.shouldAddToIgnoreList(module),
    path: options?.getSourceUrl?.(module) ?? module.path,
    source: options.excludeSource ? "" : getModuleSource(module),
  };
}
function getModuleSource(module) {
  if ((0, _js.getJsOutput)(module).type === "js/module/asset") {
    return "";
  }
  return module.getSource().toString();
}
