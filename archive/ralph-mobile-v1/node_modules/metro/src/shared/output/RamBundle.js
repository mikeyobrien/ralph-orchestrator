"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.build = build;
exports.formatName = void 0;
exports.save = save;
var _Server = _interopRequireDefault(require("../../Server"));
var _asAssets = _interopRequireDefault(require("./RamBundle/as-assets"));
var _asIndexedFile = require("./RamBundle/as-indexed-file");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
async function build(packagerClient, requestOptions) {
  const options = {
    ..._Server.default.DEFAULT_BUNDLE_OPTIONS,
    ...requestOptions,
  };
  return await packagerClient.getRamBundleInfo(options);
}
function save(bundle, options, log) {
  return options.platform === "android" && !(options.indexedRamBundle === true)
    ? (0, _asAssets.default)(bundle, options, log)
    : (0, _asIndexedFile.save)(bundle, options, log);
}
const formatName = (exports.formatName = "bundle");
