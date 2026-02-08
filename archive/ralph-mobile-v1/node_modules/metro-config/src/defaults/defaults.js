"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.assetResolutions =
  exports.assetExts =
  exports.additionalExts =
  exports.DEFAULT_METRO_MINIFIER_PATH =
    void 0;
Object.defineProperty(exports, "defaultCreateModuleIdFactory", {
  enumerable: true,
  get: function () {
    return _createModuleIdFactory.default;
  },
});
exports.sourceExts =
  exports.platforms =
  exports.noopPerfLoggerFactory =
  exports.moduleSystem =
    void 0;
var _createModuleIdFactory = _interopRequireDefault(
  require("./createModuleIdFactory"),
);
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const assetExts = (exports.assetExts = [
  "bmp",
  "gif",
  "jpg",
  "jpeg",
  "png",
  "psd",
  "svg",
  "webp",
  "xml",
  "m4v",
  "mov",
  "mp4",
  "mpeg",
  "mpg",
  "webm",
  "aac",
  "aiff",
  "caf",
  "m4a",
  "mp3",
  "wav",
  "html",
  "pdf",
  "yaml",
  "yml",
  "otf",
  "ttf",
  "zip",
]);
const assetResolutions = (exports.assetResolutions = [
  "1",
  "1.5",
  "2",
  "3",
  "4",
]);
const sourceExts = (exports.sourceExts = ["js", "jsx", "json", "ts", "tsx"]);
const additionalExts = (exports.additionalExts = ["cjs", "mjs"]);
const moduleSystem = (exports.moduleSystem = require.resolve(
  "metro-runtime/src/polyfills/require.js",
));
const platforms = (exports.platforms = ["ios", "android", "windows", "web"]);
const DEFAULT_METRO_MINIFIER_PATH = (exports.DEFAULT_METRO_MINIFIER_PATH =
  "metro-minify-terser");
const noopPerfLoggerFactory = () => {
  class Logger {
    start() {}
    end() {}
    annotate() {}
    point() {}
    subSpan() {
      return this;
    }
  }
  return new Logger();
};
exports.noopPerfLoggerFactory = noopPerfLoggerFactory;
