"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.Graph = void 0;
var _contextModule = require("../lib/contextModule");
var _CountingSet = _interopRequireDefault(require("../lib/CountingSet"));
var _isResolvedDependency = require("../lib/isResolvedDependency");
var _buildSubgraph = require("./buildSubgraph");
var _invariant = _interopRequireDefault(require("invariant"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function getInternalOptions({ transform, resolve, onProgress, lazy, shallow }) {
  let numProcessed = 0;
  let total = 0;
  return {
    lazy,
    transform,
    resolve,
    onDependencyAdd: () => onProgress && onProgress(numProcessed, ++total),
    onDependencyAdded: () => onProgress && onProgress(++numProcessed, total),
    shallow,
  };
}
function isWeakOrLazy(dependency, options) {
  const asyncType = dependency.data.data.asyncType;
  return asyncType === "weak" || (asyncType != null && options.lazy);
}
class Graph {
  dependencies = new Map();
  #importBundleNodes = new Map();
  #gc = {
    color: new Map(),
    possibleCycleRoots: new Set(),
  };
  #resolvedContexts = new Map();
  constructor(options) {
    this.entryPoints = options.entryPoints;
    this.transformOptions = options.transformOptions;
  }
  async traverseDependencies(paths, options) {
    const internalOptions = getInternalOptions(options);
    const modifiedPathsInBaseGraph = new Set(
      paths.filter((path) => this.dependencies.has(path)),
    );
    const allModifiedPaths = new Set(paths);
    const delta = await this._buildDelta(
      modifiedPathsInBaseGraph,
      internalOptions,
      (absolutePath) =>
        !this.dependencies.has(absolutePath) ||
        allModifiedPaths.has(absolutePath),
    );
    if (delta.errors.size > 0) {
      for (const modified of modifiedPathsInBaseGraph) {
        delta.baseModuleData.set(
          modified,
          this._moduleSnapshot(
            (0, _nullthrows.default)(this.dependencies.get(modified)),
          ),
        );
      }
    }
    for (const modified of modifiedPathsInBaseGraph) {
      if (delta.errors.has(modified)) {
        continue;
      }
      const module = this.dependencies.get(modified);
      if (module == null) {
        continue;
      }
      this._recursivelyCommitModule(modified, delta, internalOptions, {
        onlyRemove: true,
      });
    }
    this._collectCycles(delta, internalOptions);
    try {
      for (const modified of modifiedPathsInBaseGraph) {
        const module = this.dependencies.get(modified);
        if (module == null) {
          continue;
        }
        this._recursivelyCommitModule(modified, delta, internalOptions);
      }
    } catch (error) {
      const rollbackDelta = {
        added: delta.added,
        deleted: delta.deleted,
        touched: new Set(),
        updatedModuleData: delta.baseModuleData,
        baseModuleData: new Map(),
        errors: new Map(),
      };
      for (const modified of modifiedPathsInBaseGraph) {
        const module = this.dependencies.get(modified);
        if (module == null) {
          continue;
        }
        this._recursivelyCommitModule(modified, rollbackDelta, internalOptions);
      }
      this._collectCycles(delta, internalOptions);
      (0, _invariant.default)(
        rollbackDelta.added.size === 0 && rollbackDelta.deleted.size === 0,
        "attempted to roll back a graph commit but there were still changes",
      );
      throw error;
    }
    const added = new Map();
    for (const path of delta.added) {
      added.set(path, (0, _nullthrows.default)(this.dependencies.get(path)));
    }
    const modified = new Map();
    for (const path of modifiedPathsInBaseGraph) {
      if (
        delta.touched.has(path) &&
        !delta.deleted.has(path) &&
        !delta.added.has(path)
      ) {
        modified.set(
          path,
          (0, _nullthrows.default)(this.dependencies.get(path)),
        );
      }
    }
    return {
      added,
      modified,
      deleted: delta.deleted,
    };
  }
  async initialTraverseDependencies(options) {
    const internalOptions = getInternalOptions(options);
    (0, _invariant.default)(
      this.dependencies.size === 0,
      "initialTraverseDependencies called on nonempty graph",
    );
    this.#gc.color.clear();
    this.#gc.possibleCycleRoots.clear();
    this.#importBundleNodes.clear();
    for (const path of this.entryPoints) {
      this.#gc.color.set(path, "black");
    }
    const delta = await this._buildDelta(this.entryPoints, internalOptions);
    if (delta.errors.size > 0) {
      throw delta.errors.values().next().value;
    }
    for (const path of this.entryPoints) {
      this._recursivelyCommitModule(path, delta, internalOptions);
    }
    this.reorderGraph({
      shallow: options.shallow,
    });
    return {
      added: this.dependencies,
      modified: new Map(),
      deleted: new Set(),
    };
  }
  async _buildDelta(pathsToVisit, options, moduleFilter) {
    const subGraph = await (0, _buildSubgraph.buildSubgraph)(
      pathsToVisit,
      this.#resolvedContexts,
      {
        resolve: options.resolve,
        transform: async (absolutePath, requireContext) => {
          options.onDependencyAdd();
          const result = await options.transform(absolutePath, requireContext);
          options.onDependencyAdded();
          return result;
        },
        shouldTraverse: (dependency) => {
          if (options.shallow || isWeakOrLazy(dependency, options)) {
            return false;
          }
          return moduleFilter == null || moduleFilter(dependency.absolutePath);
        },
      },
    );
    return {
      added: new Set(),
      touched: new Set(),
      deleted: new Set(),
      updatedModuleData: subGraph.moduleData,
      baseModuleData: new Map(),
      errors: subGraph.errors,
    };
  }
  _recursivelyCommitModule(
    path,
    delta,
    options,
    commitOptions = {
      onlyRemove: false,
    },
  ) {
    if (delta.errors.has(path)) {
      throw delta.errors.get(path);
    }
    const previousModule = this.dependencies.get(path);
    const currentModule = (0, _nullthrows.default)(
      delta.updatedModuleData.get(path) ?? delta.baseModuleData.get(path),
    );
    const previousDependencies = previousModule?.dependencies ?? new Map();
    const {
      dependencies: currentDependencies,
      resolvedContexts,
      ...transformResult
    } = currentModule;
    const nextModule = {
      ...(previousModule ?? {
        inverseDependencies: new _CountingSet.default(),
        path,
      }),
      ...transformResult,
      dependencies: new Map(previousDependencies),
    };
    this.dependencies.set(nextModule.path, nextModule);
    if (previousModule == null) {
      if (delta.deleted.has(path)) {
        delta.deleted.delete(path);
      } else {
        delta.added.add(path);
      }
    }
    let dependenciesRemoved = false;
    for (const [key, prevDependency] of previousDependencies) {
      const curDependency = currentDependencies.get(key);
      if (
        !curDependency ||
        !dependenciesEqual(prevDependency, curDependency, options)
      ) {
        dependenciesRemoved = true;
        this._removeDependency(nextModule, key, prevDependency, delta, options);
      }
    }
    let dependenciesAdded = false;
    if (!commitOptions.onlyRemove) {
      for (const [key, curDependency] of currentDependencies) {
        const prevDependency = previousDependencies.get(key);
        if (
          !prevDependency ||
          !dependenciesEqual(prevDependency, curDependency, options)
        ) {
          dependenciesAdded = true;
          this._addDependency(
            nextModule,
            key,
            curDependency,
            resolvedContexts.get(key),
            delta,
            options,
          );
        }
      }
    }
    const previousDependencyKeys = [...previousDependencies.keys()];
    const dependencyKeysChangedOrReordered =
      currentDependencies.size !== previousDependencies.size ||
      [...currentDependencies.keys()].some(
        (currentKey, index) => currentKey !== previousDependencyKeys[index],
      );
    if (
      previousModule != null &&
      !transformOutputMayDiffer(previousModule, nextModule) &&
      !dependenciesRemoved &&
      !dependenciesAdded &&
      !dependencyKeysChangedOrReordered
    ) {
      this.dependencies.set(previousModule.path, previousModule);
      return previousModule;
    }
    delta.touched.add(path);
    if (commitOptions.onlyRemove) {
      return nextModule;
    }
    (0, _invariant.default)(
      nextModule.dependencies.size === currentDependencies.size,
      "Failed to add the correct dependencies",
    );
    nextModule.dependencies = new Map(currentDependencies);
    return nextModule;
  }
  _addDependency(
    parentModule,
    key,
    dependency,
    requireContext,
    delta,
    options,
  ) {
    if (options.shallow) {
    } else if (!(0, _isResolvedDependency.isResolvedDependency)(dependency)) {
    } else if (dependency.data.data.asyncType === "weak") {
    } else if (options.lazy && dependency.data.data.asyncType != null) {
      this._incrementImportBundleReference(dependency, parentModule);
    } else {
      const path = dependency.absolutePath;
      let module = this.dependencies.get(path);
      if (!module) {
        try {
          module = this._recursivelyCommitModule(path, delta, options);
        } catch (error) {
          const module = this.dependencies.get(path);
          if (module) {
            if (module.inverseDependencies.size > 0) {
              this._markAsPossibleCycleRoot(module);
            } else {
              this._releaseModule(module, delta, options);
            }
          }
          throw error;
        }
      }
      module.inverseDependencies.add(parentModule.path);
      this._markModuleInUse(module);
    }
    if ((0, _isResolvedDependency.isResolvedDependency)(dependency)) {
      const path = dependency.absolutePath;
      if (requireContext) {
        this.#resolvedContexts.set(path, requireContext);
      } else {
        this.#resolvedContexts.delete(path);
      }
    }
    parentModule.dependencies.set(key, dependency);
  }
  _removeDependency(parentModule, key, dependency, delta, options) {
    parentModule.dependencies.delete(key);
    if (
      !(0, _isResolvedDependency.isResolvedDependency)(dependency) ||
      dependency.data.data.asyncType === "weak"
    ) {
      return;
    }
    const { absolutePath } = dependency;
    const module = this.dependencies.get(absolutePath);
    if (options.lazy && dependency.data.data.asyncType != null) {
      this._decrementImportBundleReference(dependency, parentModule);
    } else if (module) {
      module.inverseDependencies.delete(parentModule.path);
    }
    if (!module) {
      return;
    }
    if (
      module.inverseDependencies.size > 0 ||
      this.entryPoints.has(absolutePath)
    ) {
      this._markAsPossibleCycleRoot(module);
    } else {
      this._releaseModule(module, delta, options);
    }
  }
  markModifiedContextModules(filePath, modifiedPaths) {
    for (const [absolutePath, context] of this.#resolvedContexts) {
      if (
        !modifiedPaths.has(absolutePath) &&
        (0, _contextModule.fileMatchesContext)(filePath, context)
      ) {
        modifiedPaths.add(absolutePath);
      }
    }
  }
  *getModifiedModulesForDeletedPath(filePath) {
    yield* this.dependencies.get(filePath)?.inverseDependencies ?? [];
    yield* this.#importBundleNodes.get(filePath)?.inverseDependencies ?? [];
  }
  reorderGraph(options) {
    const orderedDependencies = new Map();
    this.entryPoints.forEach((entryPoint) => {
      const mainModule = this.dependencies.get(entryPoint);
      if (!mainModule) {
        throw new ReferenceError(
          "Module not registered in graph: " + entryPoint,
        );
      }
      this._reorderDependencies(mainModule, orderedDependencies, options);
    });
    this.dependencies.clear();
    for (const [key, dep] of orderedDependencies) {
      this.dependencies.set(key, dep);
    }
  }
  _reorderDependencies(module, orderedDependencies, options) {
    if (module.path) {
      if (orderedDependencies.has(module.path)) {
        return;
      }
      orderedDependencies.set(module.path, module);
    }
    module.dependencies.forEach((dependency) => {
      const path = dependency.absolutePath;
      if (path == null) {
        return;
      }
      const childModule = this.dependencies.get(path);
      if (!childModule) {
        if (dependency.data.data.asyncType != null || options.shallow) {
          return;
        } else {
          throw new ReferenceError("Module not registered in graph: " + path);
        }
      }
      this._reorderDependencies(childModule, orderedDependencies, options);
    });
  }
  _incrementImportBundleReference(dependency, parentModule) {
    const { absolutePath } = dependency;
    const importBundleNode = this.#importBundleNodes.get(absolutePath) ?? {
      inverseDependencies: new _CountingSet.default(),
    };
    importBundleNode.inverseDependencies.add(parentModule.path);
    this.#importBundleNodes.set(absolutePath, importBundleNode);
  }
  _decrementImportBundleReference(dependency, parentModule) {
    const { absolutePath } = dependency;
    const importBundleNode = (0, _nullthrows.default)(
      this.#importBundleNodes.get(absolutePath),
    );
    (0, _invariant.default)(
      importBundleNode.inverseDependencies.has(parentModule.path),
      "lazy: import bundle inverse references",
    );
    importBundleNode.inverseDependencies.delete(parentModule.path);
    if (importBundleNode.inverseDependencies.size === 0) {
      this.#importBundleNodes.delete(absolutePath);
    }
  }
  _markModuleInUse(module) {
    this.#gc.color.set(module.path, "black");
  }
  *_children(module, options) {
    for (const dependency of module.dependencies.values()) {
      if (
        !(0, _isResolvedDependency.isResolvedDependency)(dependency) ||
        isWeakOrLazy(dependency, options)
      ) {
        continue;
      }
      yield (0, _nullthrows.default)(
        this.dependencies.get(dependency.absolutePath),
      );
    }
  }
  _moduleSnapshot(module) {
    const { dependencies, getSource, output, unstable_transformResultKey } =
      module;
    const resolvedContexts = new Map();
    for (const [key, dependency] of dependencies) {
      if (!(0, _isResolvedDependency.isResolvedDependency)(dependency)) {
        continue;
      }
      const resolvedContext = this.#resolvedContexts.get(
        dependency.absolutePath,
      );
      if (resolvedContext != null) {
        resolvedContexts.set(key, resolvedContext);
      }
    }
    return {
      dependencies: new Map(dependencies),
      resolvedContexts,
      getSource,
      output,
      unstable_transformResultKey,
    };
  }
  _releaseModule(module, delta, options) {
    if (
      !delta.updatedModuleData.has(module.path) &&
      !delta.baseModuleData.has(module.path)
    ) {
      delta.baseModuleData.set(module.path, this._moduleSnapshot(module));
    }
    for (const [key, dependency] of module.dependencies) {
      if (!(0, _isResolvedDependency.isResolvedDependency)(dependency)) {
        continue;
      }
      this._removeDependency(module, key, dependency, delta, options);
    }
    this.#gc.color.set(module.path, "black");
    this._freeModule(module, delta);
  }
  _freeModule(module, delta) {
    if (delta.added.has(module.path)) {
      delta.added.delete(module.path);
    } else {
      delta.deleted.add(module.path);
    }
    this.dependencies.delete(module.path);
    this.#gc.possibleCycleRoots.delete(module.path);
    this.#gc.color.delete(module.path);
    this.#resolvedContexts.delete(module.path);
  }
  _markAsPossibleCycleRoot(module) {
    if (this.#gc.color.get(module.path) !== "purple") {
      this.#gc.color.set(module.path, "purple");
      this.#gc.possibleCycleRoots.add(module.path);
    }
  }
  _collectCycles(delta, options) {
    for (const path of this.#gc.possibleCycleRoots) {
      const module = (0, _nullthrows.default)(this.dependencies.get(path));
      const color = (0, _nullthrows.default)(this.#gc.color.get(path));
      if (color === "purple") {
        this._markGray(module, options);
      } else {
        this.#gc.possibleCycleRoots.delete(path);
        if (
          color === "black" &&
          module.inverseDependencies.size === 0 &&
          !this.entryPoints.has(path)
        ) {
          this._freeModule(module, delta);
        }
      }
    }
    for (const path of this.#gc.possibleCycleRoots) {
      const module = (0, _nullthrows.default)(this.dependencies.get(path));
      this._scan(module, options);
    }
    for (const path of this.#gc.possibleCycleRoots) {
      this.#gc.possibleCycleRoots.delete(path);
      const module = (0, _nullthrows.default)(this.dependencies.get(path));
      this._collectWhite(module, delta);
    }
  }
  _markGray(module, options) {
    const color = (0, _nullthrows.default)(this.#gc.color.get(module.path));
    if (color !== "gray") {
      this.#gc.color.set(module.path, "gray");
      for (const childModule of this._children(module, options)) {
        childModule.inverseDependencies.delete(module.path);
        this._markGray(childModule, options);
      }
    }
  }
  _scan(module, options) {
    const color = (0, _nullthrows.default)(this.#gc.color.get(module.path));
    if (color === "gray") {
      if (
        module.inverseDependencies.size > 0 ||
        this.entryPoints.has(module.path)
      ) {
        this._scanBlack(module, options);
      } else {
        this.#gc.color.set(module.path, "white");
        for (const childModule of this._children(module, options)) {
          this._scan(childModule, options);
        }
      }
    }
  }
  _scanBlack(module, options) {
    this.#gc.color.set(module.path, "black");
    for (const childModule of this._children(module, options)) {
      childModule.inverseDependencies.add(module.path);
      const childColor = (0, _nullthrows.default)(
        this.#gc.color.get(childModule.path),
      );
      if (childColor !== "black") {
        this._scanBlack(childModule, options);
      }
    }
  }
  _collectWhite(module, delta) {
    const color = (0, _nullthrows.default)(this.#gc.color.get(module.path));
    if (color === "white" && !this.#gc.possibleCycleRoots.has(module.path)) {
      this.#gc.color.set(module.path, "black");
      for (const dependency of module.dependencies.values()) {
        if (!(0, _isResolvedDependency.isResolvedDependency)(dependency)) {
          continue;
        }
        const childModule = this.dependencies.get(dependency.absolutePath);
        if (childModule) {
          this._collectWhite(childModule, delta);
        }
      }
      this._freeModule(module, delta);
    }
  }
}
exports.Graph = Graph;
function dependenciesEqual(a, b, options) {
  return (
    a === b ||
    (a.absolutePath === b.absolutePath &&
      (!options.lazy || a.data.data.asyncType === b.data.data.asyncType) &&
      contextParamsEqual(a.data.data.contextParams, b.data.data.contextParams))
  );
}
function contextParamsEqual(a, b) {
  return (
    a === b ||
    (a == null && b == null) ||
    (a != null &&
      b != null &&
      a.recursive === b.recursive &&
      a.filter.pattern === b.filter.pattern &&
      a.filter.flags === b.filter.flags &&
      a.mode === b.mode)
  );
}
function transformOutputMayDiffer(a, b) {
  return (
    a.unstable_transformResultKey == null ||
    a.unstable_transformResultKey !== b.unstable_transformResultKey
  );
}
