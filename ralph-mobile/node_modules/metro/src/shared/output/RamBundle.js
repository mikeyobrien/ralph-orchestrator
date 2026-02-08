"use strict";

const Server = require("../../Server");
const asAssets = require("./RamBundle/as-assets");
const asIndexedFile = require("./RamBundle/as-indexed-file").save;
async function build(packagerClient, requestOptions) {
  const options = {
    ...Server.DEFAULT_BUNDLE_OPTIONS,
    ...requestOptions,
    bundleType: "ram",
  };
  return await packagerClient.getRamBundleInfo(options);
}
function save(bundle, options, log) {
  return options.platform === "android" && !(options.indexedRamBundle === true)
    ? asAssets(bundle, options, log)
    : asIndexedFile(bundle, options, log);
}
exports.build = build;
exports.save = save;
exports.formatName = "bundle";
