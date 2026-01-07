"use strict";

const { isJsModule, wrapModule } = require("./helpers/js");
const jscSafeUrl = require("jsc-safe-url");
const { addParamsToDefineCall } = require("metro-transform-plugins");
const path = require("path");
const url = require("url");
function generateModules(sourceModules, graph, options) {
  const modules = [];
  for (const module of sourceModules) {
    if (isJsModule(module)) {
      const getURL = (extension) => {
        const moduleUrl = url.parse(url.format(options.clientUrl), true);
        moduleUrl.search = "";
        moduleUrl.pathname = path.relative(
          options.serverRoot ?? options.projectRoot,
          path.join(
            path.dirname(module.path),
            path.basename(module.path, path.extname(module.path)) +
              "." +
              extension
          )
        );
        delete moduleUrl.query.excludeSource;
        return url.format(moduleUrl);
      };
      const sourceMappingURL = getURL("map");
      const sourceURL = jscSafeUrl.toJscSafeUrl(getURL("bundle"));
      const code =
        prepareModule(module, graph, options) +
        `\n//# sourceMappingURL=${sourceMappingURL}\n` +
        `//# sourceURL=${sourceURL}\n`;
      modules.push({
        module: [options.createModuleId(module.path), code],
        sourceMappingURL,
        sourceURL,
      });
    }
  }
  return modules;
}
function prepareModule(module, graph, options) {
  const code = wrapModule(module, {
    ...options,
    sourceUrl: url.format(options.clientUrl),
    dev: true,
  });
  const inverseDependencies = getInverseDependencies(module.path, graph);
  const inverseDependenciesById = Object.create(null);
  Object.keys(inverseDependencies).forEach((path) => {
    inverseDependenciesById[options.createModuleId(path)] = inverseDependencies[
      path
    ].map(options.createModuleId);
  });
  return addParamsToDefineCall(code, inverseDependenciesById);
}
function getInverseDependencies(path, graph, inverseDependencies = {}) {
  if (path in inverseDependencies) {
    return inverseDependencies;
  }
  const module = graph.dependencies.get(path);
  if (!module) {
    return inverseDependencies;
  }
  inverseDependencies[path] = [];
  for (const inverse of module.inverseDependencies) {
    inverseDependencies[path].push(inverse);
    getInverseDependencies(inverse, graph, inverseDependencies);
  }
  return inverseDependencies;
}
function hmrJSBundle(delta, graph, options) {
  return {
    added: generateModules(delta.added.values(), graph, options),
    modified: generateModules(delta.modified.values(), graph, options),
    deleted: [...delta.deleted].map((path) => options.createModuleId(path)),
  };
}
module.exports = hmrJSBundle;
