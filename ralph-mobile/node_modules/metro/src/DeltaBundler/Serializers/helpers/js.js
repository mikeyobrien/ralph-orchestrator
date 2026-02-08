"use strict";

const invariant = require("invariant");
const jscSafeUrl = require("jsc-safe-url");
const { addParamsToDefineCall } = require("metro-transform-plugins");
const path = require("path");
function wrapModule(module, options) {
  const output = getJsOutput(module);
  if (output.type.startsWith("js/script")) {
    return output.data.code;
  }
  const params = getModuleParams(module, options);
  return addParamsToDefineCall(output.data.code, ...params);
}
function getModuleParams(module, options) {
  const moduleId = options.createModuleId(module.path);
  const paths = {};
  let hasPaths = false;
  const dependencyMapArray = Array.from(module.dependencies.values()).map(
    (dependency) => {
      const id = options.createModuleId(dependency.absolutePath);
      if (options.includeAsyncPaths && dependency.data.data.asyncType != null) {
        hasPaths = true;
        invariant(
          options.sourceUrl != null,
          "sourceUrl is required when includeAsyncPaths is true"
        );
        const { searchParams } = new URL(
          jscSafeUrl.toNormalUrl(options.sourceUrl)
        );
        searchParams.set("modulesOnly", "true");
        searchParams.set("runModule", "false");
        const bundlePath = path.relative(
          options.serverRoot,
          dependency.absolutePath
        );
        paths[id] =
          "/" +
          path.join(
            path.dirname(bundlePath),
            path.basename(bundlePath, path.extname(bundlePath))
          ) +
          ".bundle?" +
          searchParams.toString();
      }
      return id;
    }
  );
  const params = [
    moduleId,
    hasPaths
      ? {
          ...dependencyMapArray,
          paths,
        }
      : dependencyMapArray,
  ];
  if (options.dev) {
    params.push(path.relative(options.projectRoot, module.path));
  }
  return params;
}
function getJsOutput(module) {
  const jsModules = module.output.filter(({ type }) => type.startsWith("js/"));
  invariant(
    jsModules.length === 1,
    `Modules must have exactly one JS output, but ${
      module.path ?? "unknown module"
    } has ${jsModules.length} JS outputs.`
  );
  const jsOutput = jsModules[0];
  invariant(
    Number.isFinite(jsOutput.data.lineCount),
    `JS output must populate lineCount, but ${
      module.path ?? "unknown module"
    } has ${jsOutput.type} output with lineCount '${jsOutput.data.lineCount}'`
  );
  return jsOutput;
}
function isJsModule(module) {
  return module.output.filter(isJsOutput).length > 0;
}
function isJsOutput(output) {
  return output.type.startsWith("js/");
}
module.exports = {
  getJsOutput,
  getModuleParams,
  isJsModule,
  wrapModule,
};
