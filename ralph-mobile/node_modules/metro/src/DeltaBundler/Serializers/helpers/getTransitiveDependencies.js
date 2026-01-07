"use strict";

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
    _getDeps(dependency.absolutePath, graph, deps);
  }
  return deps;
}
module.exports = getTransitiveDependencies;
