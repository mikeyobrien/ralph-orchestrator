"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = getRamBundleInfo;
var _util = require("../../Bundler/util");
var _getAppendScripts = _interopRequireDefault(
  require("../../lib/getAppendScripts"),
);
var _getTransitiveDependencies = _interopRequireDefault(
  require("./helpers/getTransitiveDependencies"),
);
var _js = require("./helpers/js");
var _sourceMapObject = require("./sourceMapObject");
var _nullthrows = _interopRequireDefault(require("nullthrows"));
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
async function getRamBundleInfo(entryPoint, pre, graph, options) {
  let modules = [...pre, ...graph.dependencies.values()];
  modules = modules.concat(
    (0, _getAppendScripts.default)(entryPoint, modules, options),
  );
  modules.forEach((module) => options.createModuleId(module.path));
  const ramModules = modules
    .filter(_js.isJsModule)
    .filter(options.processModuleFilter)
    .map((module) => ({
      id: options.createModuleId(module.path),
      code: (0, _js.wrapModule)(module, options),
      map: (0, _sourceMapObject.sourceMapObject)([module], {
        excludeSource: options.excludeSource,
        processModuleFilter: options.processModuleFilter,
        shouldAddToIgnoreList: options.shouldAddToIgnoreList,
        getSourceUrl: options.getSourceUrl,
      }),
      name: _path.default.basename(module.path),
      sourcePath: module.path,
      source: module.getSource().toString(),
      type: (0, _nullthrows.default)(
        module.output.find(({ type }) => type.startsWith("js")),
      ).type,
    }));
  const { preloadedModules, ramGroups } = await _getRamOptions(
    entryPoint,
    {
      dev: options.dev,
      platform: options.platform,
    },
    (filePath) => (0, _getTransitiveDependencies.default)(filePath, graph),
    options.getTransformOptions,
  );
  const startupModules = [];
  const lazyModules = [];
  ramModules.forEach((module) => {
    if (preloadedModules.hasOwnProperty(module.sourcePath)) {
      startupModules.push(module);
      return;
    }
    if (module.type.startsWith("js/script")) {
      startupModules.push(module);
      return;
    }
    if (module.type.startsWith("js/module")) {
      lazyModules.push(module);
    }
  });
  const groups = (0, _util.createRamBundleGroups)(
    ramGroups,
    lazyModules,
    (module, dependenciesByPath) => {
      const deps = (0, _getTransitiveDependencies.default)(
        module.sourcePath,
        graph,
      );
      const output = new Set();
      for (const dependency of deps) {
        const module = dependenciesByPath.get(dependency);
        if (module) {
          output.add(module.id);
        }
      }
      return output;
    },
  );
  return {
    getDependencies: (filePath) =>
      (0, _getTransitiveDependencies.default)(filePath, graph),
    groups,
    lazyModules,
    startupModules,
  };
}
async function _getRamOptions(
  entryFile,
  options,
  getDependencies,
  getTransformOptions,
) {
  if (getTransformOptions == null) {
    return {
      preloadedModules: {},
      ramGroups: [],
    };
  }
  const { preloadedModules, ramGroups } = await getTransformOptions(
    [entryFile],
    {
      dev: options.dev,
      hot: true,
      platform: options.platform,
    },
    async (x) => Array.from(getDependencies),
  );
  return {
    preloadedModules: preloadedModules || {},
    ramGroups: ramGroups || [],
  };
}
