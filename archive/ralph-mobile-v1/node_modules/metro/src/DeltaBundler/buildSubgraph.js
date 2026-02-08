"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.buildSubgraph = buildSubgraph;
var _contextModule = require("../lib/contextModule");
var _isResolvedDependency = require("../lib/isResolvedDependency");
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function resolveDependencies(parentPath, dependencies, resolve) {
  const maybeResolvedDeps = new Map();
  const resolvedContexts = new Map();
  for (const dep of dependencies) {
    let maybeResolvedDep;
    const key = dep.data.key;
    const { contextParams } = dep.data;
    if (contextParams) {
      const from = _path.default.join(parentPath, "..", dep.name);
      const absolutePath = (0, _contextModule.deriveAbsolutePathFromContext)(
        from,
        contextParams,
      );
      const resolvedContext = {
        from,
        mode: contextParams.mode,
        recursive: contextParams.recursive,
        filter: new RegExp(
          contextParams.filter.pattern,
          contextParams.filter.flags,
        ),
      };
      resolvedContexts.set(key, resolvedContext);
      maybeResolvedDep = {
        absolutePath,
        data: dep,
      };
    } else {
      try {
        maybeResolvedDep = {
          absolutePath: resolve(parentPath, dep).filePath,
          data: dep,
        };
      } catch (error) {
        if (dep.data.isOptional !== true) {
          throw error;
        }
        maybeResolvedDep = {
          data: dep,
        };
      }
    }
    if (maybeResolvedDeps.has(key)) {
      throw new Error(
        `resolveDependencies: Found duplicate dependency key '${key}' in ${parentPath}`,
      );
    }
    maybeResolvedDeps.set(key, maybeResolvedDep);
  }
  return {
    dependencies: maybeResolvedDeps,
    resolvedContexts,
  };
}
async function buildSubgraph(
  entryPaths,
  resolvedContexts,
  { resolve, transform, shouldTraverse },
) {
  const moduleData = new Map();
  const errors = new Map();
  const visitedPaths = new Set();
  async function visit(absolutePath, requireContext) {
    if (visitedPaths.has(absolutePath)) {
      return;
    }
    visitedPaths.add(absolutePath);
    const transformResult = await transform(absolutePath, requireContext);
    const resolutionResult = resolveDependencies(
      absolutePath,
      transformResult.dependencies,
      resolve,
    );
    moduleData.set(absolutePath, {
      ...transformResult,
      ...resolutionResult,
    });
    await Promise.all(
      [...resolutionResult.dependencies.values()]
        .filter(
          (dependency) =>
            (0, _isResolvedDependency.isResolvedDependency)(dependency) &&
            shouldTraverse(dependency),
        )
        .map((dependency) =>
          visit(
            dependency.absolutePath,
            resolutionResult.resolvedContexts.get(dependency.data.data.key),
          ).catch((error) => errors.set(dependency.absolutePath, error)),
        ),
    );
  }
  await Promise.all(
    [...entryPaths].map((absolutePath) =>
      visit(absolutePath, resolvedContexts.get(absolutePath)).catch((error) =>
        errors.set(absolutePath, error),
      ),
    ),
  );
  return {
    moduleData,
    errors,
  };
}
