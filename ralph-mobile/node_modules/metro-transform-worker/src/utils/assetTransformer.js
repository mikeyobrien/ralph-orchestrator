"use strict";

const { getAssetData } = require("metro/src/Assets");
const { generateAssetCodeFileAst } = require("metro/src/Bundler/util");
const path = require("path");
async function transform(
  { filename, options, src },
  assetRegistryPath,
  assetDataPlugins
) {
  options = options || {
    platform: "",
    projectRoot: "",
    inlineRequires: false,
    minify: false,
  };
  const absolutePath = path.resolve(options.projectRoot, filename);
  const data = await getAssetData(
    absolutePath,
    filename,
    assetDataPlugins,
    options.platform,
    options.publicPath
  );
  return {
    ast: generateAssetCodeFileAst(assetRegistryPath, data),
  };
}
module.exports = {
  transform,
};
