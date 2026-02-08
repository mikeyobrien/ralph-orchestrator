"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = saveAsAssets;
var _relativizeSourceMap = _interopRequireDefault(
  require("../../../lib/relativizeSourceMap"),
);
var _writeFile = _interopRequireDefault(require("../writeFile"));
var _buildSourcemapWithMetadata = _interopRequireDefault(
  require("./buildSourcemapWithMetadata"),
);
var _magicNumber = _interopRequireDefault(require("./magic-number"));
var _util = require("./util");
var _writeSourcemap = _interopRequireDefault(require("./write-sourcemap"));
var _fs = require("fs");
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const MAGIC_RAM_BUNDLE_FILENAME = "UNBUNDLE";
const MODULES_DIR = "js-modules";
function saveAsAssets(bundle, options, log) {
  const {
    bundleOutput,
    bundleEncoding: encoding,
    sourcemapOutput,
    sourcemapSourcesRoot,
  } = options;
  log("start");
  const { startupModules, lazyModules } = bundle;
  log("finish");
  const startupCode = (0, _util.joinModules)(startupModules);
  log("Writing bundle output to:", bundleOutput);
  const modulesDir = _path.default.join(
    _path.default.dirname(bundleOutput),
    MODULES_DIR,
  );
  const writeUnbundle = createDir(modulesDir).then(() =>
    Promise.all([
      writeModules(lazyModules, modulesDir, encoding),
      (0, _writeFile.default)(bundleOutput, startupCode, encoding),
      writeMagicFlagFile(modulesDir),
    ]),
  );
  writeUnbundle.then(() => log("Done writing unbundle output"));
  if (sourcemapOutput) {
    const sourceMap = (0, _buildSourcemapWithMetadata.default)({
      fixWrapperOffset: true,
      lazyModules: lazyModules.concat(),
      moduleGroups: null,
      startupModules: startupModules.concat(),
    });
    if (sourcemapSourcesRoot != null) {
      (0, _relativizeSourceMap.default)(sourceMap, sourcemapSourcesRoot);
    }
    const wroteSourceMap = (0, _writeSourcemap.default)(
      sourcemapOutput,
      JSON.stringify(sourceMap),
      log,
    );
    return Promise.all([writeUnbundle, wroteSourceMap]);
  } else {
    return writeUnbundle;
  }
}
function createDir(dirName) {
  return _fs.promises.mkdir(dirName, {
    recursive: true,
  });
}
function writeModuleFile(module, modulesDir, encoding) {
  const { code, id } = module;
  return (0, _writeFile.default)(
    _path.default.join(modulesDir, id + ".js"),
    code,
    encoding,
  );
}
function writeModules(modules, modulesDir, encoding) {
  const writeFiles = modules.map((module) =>
    writeModuleFile(module, modulesDir, encoding),
  );
  return Promise.all(writeFiles);
}
function writeMagicFlagFile(outputDir) {
  const buffer = Buffer.alloc(4);
  buffer.writeUInt32LE(_magicNumber.default, 0);
  return (0, _writeFile.default)(
    _path.default.join(outputDir, MAGIC_RAM_BUNDLE_FILENAME),
    buffer,
  );
}
