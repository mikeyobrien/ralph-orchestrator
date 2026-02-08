"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.transform = transform;
var _Assets = require("metro/private/Assets");
var _util = require("metro/private/Bundler/util");
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
async function transform(
  { filename, options, src },
  assetRegistryPath,
  assetDataPlugins,
) {
  options = options || {
    platform: "",
    projectRoot: "",
    inlineRequires: false,
    minify: false,
  };
  const absolutePath = _path.default.resolve(options.projectRoot, filename);
  const data = await (0, _Assets.getAssetData)(
    absolutePath,
    filename,
    assetDataPlugins,
    options.platform,
    options.publicPath,
  );
  return {
    ast: (0, _util.generateAssetCodeFileAst)(assetRegistryPath, data),
  };
}
