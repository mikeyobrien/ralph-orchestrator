"use strict";

const relativizeSourceMapInline = require("../../lib/relativizeSourceMap");
const Server = require("../../Server");
const writeFile = require("./writeFile");
function buildBundle(packagerClient, requestOptions) {
  return packagerClient.build({
    ...Server.DEFAULT_BUNDLE_OPTIONS,
    ...requestOptions,
    bundleType: "bundle",
  });
}
function relativateSerializedMap(map, sourceMapSourcesRoot) {
  const sourceMap = JSON.parse(map);
  relativizeSourceMapInline(sourceMap, sourceMapSourcesRoot);
  return JSON.stringify(sourceMap);
}
async function saveBundleAndMap(bundle, options, log) {
  const {
    bundleOutput,
    bundleEncoding: encoding,
    sourcemapOutput,
    sourcemapSourcesRoot,
  } = options;
  const writeFns = [];
  writeFns.push(async () => {
    log(`Writing bundle output to: ${bundleOutput}`);
    await writeFile(bundleOutput, bundle.code, encoding);
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
      await writeFile(sourcemapOutput, map);
      log("Done writing sourcemap output");
    });
  }
  await Promise.all(writeFns.map((cb) => cb()));
}
exports.build = buildBundle;
exports.save = saveBundleAndMap;
exports.formatName = "bundle";
