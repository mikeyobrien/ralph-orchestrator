"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = getTransitiveDependencies;
var _isResolvedDependency = require("../../../lib/isResolvedDependency");
function getTransitiveDependencies(path, graph) {
  const dependencies = _getDeps(path, graph, new Set());
  dependencies.delete(path);
  return dependencies;
}
function _getDeps(path, graph, deps) {
  if (deps.has(path)) {
    return deps;
  }
  const module = graph.dependencies.get(path);
  if (!module) {
    return deps;
  }
  deps.add(path);
  for (const dependency of module.dependencies.values()) {
    if ((0, _isResolvedDependency.isResolvedDependency)(dependency)) {
      _getDeps(dependency.absolutePath, graph, deps);
    }
  }
  return deps;
}
