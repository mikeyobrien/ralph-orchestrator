"use strict";

const defaultCreateModuleIdFactory = require("metro/src/lib/createModuleIdFactory");
exports.assetExts = [
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
];
exports.assetResolutions = ["1", "1.5", "2", "3", "4"];
exports.sourceExts = ["js", "jsx", "json", "ts", "tsx"];
exports.additionalExts = ["cjs", "mjs"];
exports.moduleSystem = require.resolve(
  "metro-runtime/src/polyfills/require.js"
);
exports.platforms = ["ios", "android", "windows", "web"];
exports.DEFAULT_METRO_MINIFIER_PATH = "metro-minify-terser";
exports.defaultCreateModuleIdFactory = defaultCreateModuleIdFactory;
exports.noopPerfLoggerFactory = () => {
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
