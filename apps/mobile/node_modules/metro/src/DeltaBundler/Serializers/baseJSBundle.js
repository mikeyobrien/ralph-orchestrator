"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = baseJSBundle;
var _getAppendScripts = _interopRequireDefault(
  require("../../lib/getAppendScripts"),
);
var _processModules = _interopRequireDefault(
  require("./helpers/processModules"),
);
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function baseJSBundle(entryPoint, preModules, graph, options) {
  for (const module of graph.dependencies.values()) {
    options.createModuleId(module.path);
  }
  const processModulesOptions = {
    filter: options.processModuleFilter,
    createModuleId: options.createModuleId,
    dev: options.dev,
    includeAsyncPaths: options.includeAsyncPaths,
    projectRoot: options.projectRoot,
    serverRoot: options.serverRoot,
    sourceUrl: options.sourceUrl,
  };
  if (options.modulesOnly) {
    preModules = [];
  }
  const preCode = (0, _processModules.default)(
    preModules,
    processModulesOptions,
  )
    .map(([_, code]) => code)
    .join("\n");
  const modules = [...graph.dependencies.values()].sort(
    (a, b) => options.createModuleId(a.path) - options.createModuleId(b.path),
  );
  const postCode = (0, _processModules.default)(
    (0, _getAppendScripts.default)(entryPoint, [...preModules, ...modules], {
      asyncRequireModulePath: options.asyncRequireModulePath,
      createModuleId: options.createModuleId,
      getRunModuleStatement: options.getRunModuleStatement,
      globalPrefix: options.globalPrefix,
      inlineSourceMap: options.inlineSourceMap,
      runBeforeMainModule: options.runBeforeMainModule,
      runModule: options.runModule,
      shouldAddToIgnoreList: options.shouldAddToIgnoreList,
      sourceMapUrl: options.sourceMapUrl,
      sourceUrl: options.sourceUrl,
      getSourceUrl: options.getSourceUrl,
    }),
    processModulesOptions,
  )
    .map(([_, code]) => code)
    .join("\n");
  return {
    pre: preCode,
    post: postCode,
    modules: (0, _processModules.default)(
      [...graph.dependencies.values()],
      processModulesOptions,
    ).map(([module, code]) => [options.createModuleId(module.path), code]),
  };
}
