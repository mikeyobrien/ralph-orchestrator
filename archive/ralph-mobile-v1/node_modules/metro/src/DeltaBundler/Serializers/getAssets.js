"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = getAssets;
var _Assets = require("../../Assets");
var _js = require("./helpers/js");
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
async function getAssets(dependencies, options) {
  const promises = [];
  const { processModuleFilter } = options;
  for (const module of dependencies.values()) {
    if (
      (0, _js.isJsModule)(module) &&
      processModuleFilter(module) &&
      (0, _js.getJsOutput)(module).type === "js/module/asset" &&
      _path.default.relative(options.projectRoot, module.path) !==
        "package.json"
    ) {
      promises.push(
        (0, _Assets.getAssetData)(
          module.path,
          _path.default.relative(options.projectRoot, module.path),
          options.assetPlugins,
          options.platform,
          options.publicPath,
        ),
      );
    }
  }
  return await Promise.all(promises);
}
