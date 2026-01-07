"use strict";

const getSourceMapInfo = require("./helpers/getSourceMapInfo");
const { isJsModule } = require("./helpers/js");
const {
  fromRawMappings,
  fromRawMappingsNonBlocking,
} = require("metro-source-map");
function getSourceMapInfosImpl(isBlocking, onDone, modules, options) {
  const sourceMapInfos = [];
  const modulesToProcess = modules
    .filter(isJsModule)
    .filter(options.processModuleFilter);
  function processNextModule() {
    if (modulesToProcess.length === 0) {
      return true;
    }
    const mod = modulesToProcess.shift();
    const info = getSourceMapInfo(mod, {
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
    options
  );
  if (sourceMapInfos == null) {
    throw new Error(
      "Expected getSourceMapInfosImpl() to finish synchronously."
    );
  }
  return fromRawMappings(sourceMapInfos);
}
async function sourceMapGeneratorNonBlocking(modules, options) {
  const sourceMapInfos = await new Promise((resolve) => {
    getSourceMapInfosImpl(false, resolve, modules, options);
  });
  return fromRawMappingsNonBlocking(sourceMapInfos);
}
module.exports = {
  sourceMapGenerator,
  sourceMapGeneratorNonBlocking,
};
