"use strict";

const relativizeSourceMapInline = require("../../../lib/relativizeSourceMap");
const writeFile = require("../writeFile");
const buildSourcemapWithMetadata = require("./buildSourcemapWithMetadata");
const MAGIC_RAM_BUNDLE_NUMBER = require("./magic-number");
const { joinModules } = require("./util");
const writeSourceMap = require("./write-sourcemap");
const fsPromises = require("fs").promises;
const path = require("path");
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
  const startupCode = joinModules(startupModules);
  log("Writing bundle output to:", bundleOutput);
  const modulesDir = path.join(path.dirname(bundleOutput), MODULES_DIR);
  const writeUnbundle = createDir(modulesDir).then(() =>
    Promise.all([
      writeModules(lazyModules, modulesDir, encoding),
      writeFile(bundleOutput, startupCode, encoding),
      writeMagicFlagFile(modulesDir),
    ])
  );
  writeUnbundle.then(() => log("Done writing unbundle output"));
  if (sourcemapOutput) {
    const sourceMap = buildSourcemapWithMetadata({
      fixWrapperOffset: true,
      lazyModules: lazyModules.concat(),
      moduleGroups: null,
      startupModules: startupModules.concat(),
    });
    if (sourcemapSourcesRoot != null) {
      relativizeSourceMapInline(sourceMap, sourcemapSourcesRoot);
    }
    const wroteSourceMap = writeSourceMap(
      sourcemapOutput,
      JSON.stringify(sourceMap),
      log
    );
    return Promise.all([writeUnbundle, wroteSourceMap]);
  } else {
    return writeUnbundle;
  }
}
function createDir(dirName) {
  return fsPromises.mkdir(dirName, {
    recursive: true,
  });
}
function writeModuleFile(module, modulesDir, encoding) {
  const { code, id } = module;
  return writeFile(path.join(modulesDir, id + ".js"), code, encoding);
}
function writeModules(modules, modulesDir, encoding) {
  const writeFiles = modules.map((module) =>
    writeModuleFile(module, modulesDir, encoding)
  );
  return Promise.all(writeFiles);
}
function writeMagicFlagFile(outputDir) {
  const buffer = Buffer.alloc(4);
  buffer.writeUInt32LE(MAGIC_RAM_BUNDLE_NUMBER, 0);
  return writeFile(path.join(outputDir, MAGIC_RAM_BUNDLE_FILENAME), buffer);
}
module.exports = saveAsAssets;
