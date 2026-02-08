"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.build = build;
exports.formatName = void 0;
exports.save = save;
var _relativizeSourceMap = _interopRequireDefault(
  require("../../lib/relativizeSourceMap"),
);
var _Server = _interopRequireDefault(require("../../Server"));
var _writeFile = _interopRequireDefault(require("./writeFile"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const DEFAULTS = _Server.default.DEFAULT_BUNDLE_OPTIONS;
function build(packagerClient, requestOptions, buildOptions = {}) {
  return packagerClient.build(
    {
      ...DEFAULTS,
      ...requestOptions,
      ...{
        customResolverOptions:
          requestOptions.customResolverOptions ??
          DEFAULTS.customResolverOptions,
        customTransformOptions:
          requestOptions.customTransformOptions ??
          DEFAULTS.customTransformOptions,
        dev: requestOptions.dev ?? DEFAULTS.dev,
        inlineSourceMap:
          requestOptions.inlineSourceMap ?? DEFAULTS.inlineSourceMap,
        unstable_transformProfile:
          requestOptions.unstable_transformProfile ??
          DEFAULTS.unstable_transformProfile,
      },
    },
    buildOptions,
  );
}
function relativateSerializedMap(map, sourceMapSourcesRoot) {
  const sourceMap = JSON.parse(map);
  (0, _relativizeSourceMap.default)(sourceMap, sourceMapSourcesRoot);
  return JSON.stringify(sourceMap);
}
async function save(bundle, options, log) {
  const {
    bundleOutput,
    bundleEncoding: encoding,
    sourcemapOutput,
    sourcemapSourcesRoot,
  } = options;
  const writeFns = [];
  writeFns.push(async () => {
    log(`Writing bundle output to: ${bundleOutput}`);
    await (0, _writeFile.default)(bundleOutput, bundle.code, encoding);
    log("Done writing bundle output");
  });
  if (sourcemapOutput) {
    let { map } = bundle;
    if (sourcemapSourcesRoot != null) {
      log("start relativating source map");
      map = relativateSerializedMap(map, sourcemapSourcesRoot);
      log("finished relativating");
    }
    writeFns.push(async () => {
      log(`Writing sourcemap output to: ${sourcemapOutput}`);
      await (0, _writeFile.default)(sourcemapOutput, map);
      log("Done writing sourcemap output");
    });
  }
  await Promise.all(writeFns.map((cb) => cb()));
}
const formatName = (exports.formatName = "bundle");
