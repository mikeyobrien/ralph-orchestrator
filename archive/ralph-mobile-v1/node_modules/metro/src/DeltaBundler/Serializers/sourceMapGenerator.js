"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.sourceMapGenerator = sourceMapGenerator;
exports.sourceMapGeneratorNonBlocking = sourceMapGeneratorNonBlocking;
var _getSourceMapInfo = _interopRequireDefault(
  require("./helpers/getSourceMapInfo"),
);
var _js = require("./helpers/js");
var _metroSourceMap = require("metro-source-map");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function getSourceMapInfosImpl(isBlocking, onDone, modules, options) {
  const sourceMapInfos = [];
  const modulesToProcess = modules
    .filter(_js.isJsModule)
    .filter(options.processModuleFilter);
  function processNextModule() {
    if (modulesToProcess.length === 0) {
      return true;
    }
    const mod = modulesToProcess.shift();
    const info = (0, _getSourceMapInfo.default)(mod, {
      excludeSource: options.excludeSource,
      shouldAddToIgnoreList: options.shouldAddToIgnoreList,
      getSourceUrl: options.getSourceUrl,
    });
    sourceMapInfos.push(info);
    return false;
  }
  function workLoop() {
    const time = process.hrtime();
    while (true) {
      const isDone = processNextModule();
      if (isDone) {
        onDone(sourceMapInfos);
        break;
      }
      if (!isBlocking) {
        const diff = process.hrtime(time);
        const NS_IN_MS = 1000000;
        if (diff[1] > 50 * NS_IN_MS) {
          setImmediate(workLoop);
          break;
        }
      }
    }
  }
  workLoop();
}
function sourceMapGenerator(modules, options) {
  let sourceMapInfos;
  getSourceMapInfosImpl(
    true,
    (infos) => {
      sourceMapInfos = infos;
    },
    modules,
    options,
  );
  if (sourceMapInfos == null) {
    throw new Error(
      "Expected getSourceMapInfosImpl() to finish synchronously.",
    );
  }
  return (0, _metroSourceMap.fromRawMappings)(sourceMapInfos);
}
async function sourceMapGeneratorNonBlocking(modules, options) {
  const sourceMapInfos = await new Promise((resolve) => {
    getSourceMapInfosImpl(false, resolve, modules, options);
  });
  return (0, _metroSourceMap.fromRawMappingsNonBlocking)(sourceMapInfos);
}
