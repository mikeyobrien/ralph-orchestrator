"use strict";

const getAppendScripts = require("../../lib/getAppendScripts");
const processModules = require("./helpers/processModules");
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
  const preCode = processModules(preModules, processModulesOptions)
    .map(([_, code]) => code)
    .join("\n");
  const modules = [...graph.dependencies.values()].sort(
    (a, b) => options.createModuleId(a.path) - options.createModuleId(b.path)
  );
  const postCode = processModules(
    getAppendScripts(entryPoint, [...preModules, ...modules], {
      asyncRequireModulePath: options.asyncRequireModulePath,
      createModuleId: options.createModuleId,
      getRunModuleStatement: options.getRunModuleStatement,
      inlineSourceMap: options.inlineSourceMap,
      runBeforeMainModule: options.runBeforeMainModule,
      runModule: options.runModule,
      shouldAddToIgnoreList: options.shouldAddToIgnoreList,
      sourceMapUrl: options.sourceMapUrl,
      sourceUrl: options.sourceUrl,
      getSourceUrl: options.getSourceUrl,
    }),
    processModulesOptions
  )
    .map(([_, code]) => code)
    .join("\n");
  return {
    pre: preCode,
    post: postCode,
    modules: processModules(
      [...graph.dependencies.values()],
      processModulesOptions
    ).map(([module, code]) => [options.createModuleId(module.path), code]),
  };
}
module.exports = baseJSBundle;
