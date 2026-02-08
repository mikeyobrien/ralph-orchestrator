"use strict";

global.__r = metroRequire;
global[`${__METRO_GLOBAL_PREFIX__}__d`] = define;
global.__c = clear;
global.__registerSegment = registerSegment;
var modules = clear();
const EMPTY = {};
const CYCLE_DETECTED = {};
const { hasOwnProperty } = {};
if (__DEV__) {
  global.$RefreshReg$ = global.$RefreshReg$ ?? (() => {});
  global.$RefreshSig$ = global.$RefreshSig$ ?? (() => (type) => type);
}
function clear() {
  modules = new Map();
  return modules;
}
if (__DEV__) {
  var verboseNamesToModuleIds = new Map();
  var getModuleIdForVerboseName = (verboseName) => {
    const moduleId = verboseNamesToModuleIds.get(verboseName);
    if (moduleId == null) {
      throw new Error(`Unknown named module: "${verboseName}"`);
    }
    return moduleId;
  };
  var initializingModuleIds = [];
}
function define(factory, moduleId, dependencyMap) {
  if (modules.has(moduleId)) {
    if (__DEV__) {
      const inverseDependencies = arguments[4];
      if (inverseDependencies) {
        global.__accept(moduleId, factory, dependencyMap, inverseDependencies);
      }
    }
    return;
  }
  const mod = {
    dependencyMap,
    factory,
    hasError: false,
    importedAll: EMPTY,
    importedDefault: EMPTY,
    isInitialized: false,
    publicModule: {
      exports: {},
    },
  };
  modules.set(moduleId, mod);
  if (__DEV__) {
    mod.hot = createHotReloadingObject();
    const verboseName = arguments[3];
    if (verboseName) {
      mod.verboseName = verboseName;
      verboseNamesToModuleIds.set(verboseName, moduleId);
    }
  }
}
function metroRequire(moduleId, maybeNameForDev) {
  if (moduleId === null) {
    if (__DEV__ && typeof maybeNameForDev === "string") {
      throw new Error("Cannot find module '" + maybeNameForDev + "'");
    }
    throw new Error("Cannot find module");
  }
  if (__DEV__ && typeof moduleId === "string") {
    const verboseName = moduleId;
    moduleId = getModuleIdForVerboseName(verboseName);
    console.warn(
      `Requiring module "${verboseName}" by name is only supported for ` +
        "debugging purposes and will BREAK IN PRODUCTION!",
    );
  }
  const moduleIdReallyIsNumber = moduleId;
  if (__DEV__) {
    const initializingIndex = initializingModuleIds.indexOf(
      moduleIdReallyIsNumber,
    );
    if (initializingIndex !== -1) {
      const cycle = initializingModuleIds
        .slice(initializingIndex)
        .map((id) => modules.get(id)?.verboseName ?? "[unknown]");
      if (shouldPrintRequireCycle(cycle)) {
        cycle.push(cycle[0]);
        console.warn(
          `Require cycle: ${cycle.join(" -> ")}\n\n` +
            "Require cycles are allowed, but can result in uninitialized values. " +
            "Consider refactoring to remove the need for a cycle.",
        );
      }
    }
  }
  const module = modules.get(moduleIdReallyIsNumber);
  return module && module.isInitialized
    ? module.publicModule.exports
    : guardedLoadModule(moduleIdReallyIsNumber, module);
}
function shouldPrintRequireCycle(modules) {
  const regExps =
    global[__METRO_GLOBAL_PREFIX__ + "__requireCycleIgnorePatterns"];
  if (!Array.isArray(regExps)) {
    return true;
  }
  const isIgnored = (module) =>
    module != null && regExps.some((regExp) => regExp.test(module));
  return modules.every((module) => !isIgnored(module));
}
function metroImportDefault(moduleId) {
  if (__DEV__ && typeof moduleId === "string") {
    const verboseName = moduleId;
    moduleId = getModuleIdForVerboseName(verboseName);
  }
  const moduleIdReallyIsNumber = moduleId;
  const maybeInitializedModule = modules.get(moduleIdReallyIsNumber);
  if (
    maybeInitializedModule &&
    maybeInitializedModule.importedDefault !== EMPTY
  ) {
    return maybeInitializedModule.importedDefault;
  }
  const exports = metroRequire(moduleIdReallyIsNumber);
  const importedDefault =
    exports && exports.__esModule ? exports.default : exports;
  const initializedModule = modules.get(moduleIdReallyIsNumber);
  return (initializedModule.importedDefault = importedDefault);
}
metroRequire.importDefault = metroImportDefault;
function metroImportAll(moduleId) {
  if (__DEV__ && typeof moduleId === "string") {
    const verboseName = moduleId;
    moduleId = getModuleIdForVerboseName(verboseName);
  }
  const moduleIdReallyIsNumber = moduleId;
  const maybeInitializedModule = modules.get(moduleIdReallyIsNumber);
  if (maybeInitializedModule && maybeInitializedModule.importedAll !== EMPTY) {
    return maybeInitializedModule.importedAll;
  }
  const exports = metroRequire(moduleIdReallyIsNumber);
  let importedAll;
  if (exports && exports.__esModule) {
    importedAll = exports;
  } else {
    importedAll = {};
    if (exports) {
      for (const key in exports) {
        if (hasOwnProperty.call(exports, key)) {
          importedAll[key] = exports[key];
        }
      }
    }
    importedAll.default = exports;
  }
  const initializedModule = modules.get(moduleIdReallyIsNumber);
  return (initializedModule.importedAll = importedAll);
}
metroRequire.importAll = metroImportAll;
metroRequire.context = function fallbackRequireContext() {
  if (__DEV__) {
    throw new Error(
      "The experimental Metro feature `require.context` is not enabled in your project.\nThis can be enabled by setting the `transformer.unstable_allowRequireContext` property to `true` in your Metro configuration.",
    );
  }
  throw new Error(
    "The experimental Metro feature `require.context` is not enabled in your project.",
  );
};
metroRequire.resolveWeak = function fallbackRequireResolveWeak() {
  if (__DEV__) {
    throw new Error(
      "require.resolveWeak cannot be called dynamically. Ensure you are using the same version of `metro` and `metro-runtime`.",
    );
  }
  throw new Error("require.resolveWeak cannot be called dynamically.");
};
let inGuard = false;
function guardedLoadModule(moduleId, module) {
  if (!inGuard && global.ErrorUtils) {
    inGuard = true;
    let returnValue;
    try {
      returnValue = loadModuleImplementation(moduleId, module);
    } catch (e) {
      global.ErrorUtils.reportFatalError(e);
    }
    inGuard = false;
    return returnValue;
  } else {
    return loadModuleImplementation(moduleId, module);
  }
}
const ID_MASK_SHIFT = 16;
const LOCAL_ID_MASK = ~0 >>> ID_MASK_SHIFT;
function unpackModuleId(moduleId) {
  const segmentId = moduleId >>> ID_MASK_SHIFT;
  const localId = moduleId & LOCAL_ID_MASK;
  return {
    segmentId,
    localId,
  };
}
metroRequire.unpackModuleId = unpackModuleId;
function packModuleId(value) {
  return (value.segmentId << ID_MASK_SHIFT) + value.localId;
}
metroRequire.packModuleId = packModuleId;
const moduleDefinersBySegmentID = [];
const definingSegmentByModuleID = new Map();
function registerSegment(segmentId, moduleDefiner, moduleIds) {
  moduleDefinersBySegmentID[segmentId] = moduleDefiner;
  if (__DEV__) {
    if (segmentId === 0 && moduleIds) {
      throw new Error(
        "registerSegment: Expected moduleIds to be null for main segment",
      );
    }
    if (segmentId !== 0 && !moduleIds) {
      throw new Error(
        "registerSegment: Expected moduleIds to be passed for segment #" +
          segmentId,
      );
    }
  }
  if (moduleIds) {
    moduleIds.forEach((moduleId) => {
      if (!modules.has(moduleId) && !definingSegmentByModuleID.has(moduleId)) {
        definingSegmentByModuleID.set(moduleId, segmentId);
      }
    });
  }
}
function loadModuleImplementation(moduleId, module) {
  if (!module && moduleDefinersBySegmentID.length > 0) {
    const segmentId = definingSegmentByModuleID.get(moduleId) ?? 0;
    const definer = moduleDefinersBySegmentID[segmentId];
    if (definer != null) {
      definer(moduleId);
      module = modules.get(moduleId);
      definingSegmentByModuleID.delete(moduleId);
    }
  }
  const nativeRequire = global.nativeRequire;
  if (!module && nativeRequire) {
    const { segmentId, localId } = unpackModuleId(moduleId);
    nativeRequire(localId, segmentId);
    module = modules.get(moduleId);
  }
  if (!module) {
    throw unknownModuleError(moduleId);
  }
  if (module.hasError) {
    throw module.error;
  }
  if (__DEV__) {
    var Systrace = requireSystrace();
    var Refresh = requireRefresh();
  }
  module.isInitialized = true;
  const { factory, dependencyMap } = module;
  if (__DEV__) {
    initializingModuleIds.push(moduleId);
  }
  try {
    if (__DEV__) {
      Systrace.beginEvent("JS_require_" + (module.verboseName || moduleId));
    }
    const moduleObject = module.publicModule;
    if (__DEV__) {
      moduleObject.hot = module.hot;
      var prevRefreshReg = global.$RefreshReg$;
      var prevRefreshSig = global.$RefreshSig$;
      if (Refresh != null) {
        const RefreshRuntime = Refresh;
        global.$RefreshReg$ = (type, id) => {
          const prefixedModuleId =
            __METRO_GLOBAL_PREFIX__ + " " + moduleId + " " + id;
          RefreshRuntime.register(type, prefixedModuleId);
        };
        global.$RefreshSig$ =
          RefreshRuntime.createSignatureFunctionForTransform;
      }
    }
    moduleObject.id = moduleId;
    factory(
      global,
      metroRequire,
      metroImportDefault,
      metroImportAll,
      moduleObject,
      moduleObject.exports,
      dependencyMap,
    );
    if (!__DEV__) {
      module.factory = undefined;
      module.dependencyMap = undefined;
    }
    if (__DEV__) {
      Systrace.endEvent();
      if (Refresh != null) {
        const prefixedModuleId = __METRO_GLOBAL_PREFIX__ + " " + moduleId;
        registerExportsForReactRefresh(
          Refresh,
          moduleObject.exports,
          prefixedModuleId,
        );
      }
    }
    return moduleObject.exports;
  } catch (e) {
    module.hasError = true;
    module.error = e;
    module.isInitialized = false;
    module.publicModule.exports = undefined;
    throw e;
  } finally {
    if (__DEV__) {
      if (initializingModuleIds.pop() !== moduleId) {
        throw new Error(
          "initializingModuleIds is corrupt; something is terribly wrong",
        );
      }
      global.$RefreshReg$ = prevRefreshReg;
      global.$RefreshSig$ = prevRefreshSig;
    }
  }
}
function unknownModuleError(id) {
  let message = 'Requiring unknown module "' + id + '".';
  if (__DEV__) {
    message +=
      " If you are sure the module exists, try restarting Metro. " +
      "You may also want to run `yarn` or `npm install`.";
  }
  return Error(message);
}
if (__DEV__) {
  metroRequire.Systrace = {
    beginEvent: () => {},
    endEvent: () => {},
  };
  metroRequire.getModules = () => {
    return modules;
  };
  var createHotReloadingObject = function () {
    const hot = {
      _acceptCallback: null,
      _disposeCallback: null,
      _didAccept: false,
      accept: (callback) => {
        hot._didAccept = true;
        hot._acceptCallback = callback;
      },
      dispose: (callback) => {
        hot._disposeCallback = callback;
      },
    };
    return hot;
  };
  let reactRefreshTimeout = null;
  const metroHotUpdateModule = function (
    id,
    factory,
    dependencyMap,
    inverseDependencies,
  ) {
    const mod = modules.get(id);
    if (!mod) {
      if (factory) {
        return;
      }
      throw unknownModuleError(id);
    }
    if (!mod.hasError && !mod.isInitialized) {
      mod.factory = factory;
      mod.dependencyMap = dependencyMap;
      return;
    }
    const Refresh = requireRefresh();
    const refreshBoundaryIDs = new Set();
    let didBailOut = false;
    let updatedModuleIDs;
    try {
      updatedModuleIDs = topologicalSort(
        [id],
        (pendingID) => {
          const pendingModule = modules.get(pendingID);
          if (pendingModule == null) {
            return [];
          }
          const pendingHot = pendingModule.hot;
          if (pendingHot == null) {
            throw new Error(
              "[Refresh] Expected module.hot to always exist in DEV.",
            );
          }
          let canAccept = pendingHot._didAccept;
          if (!canAccept && Refresh != null) {
            const isBoundary = isReactRefreshBoundary(
              Refresh,
              pendingModule.publicModule.exports,
            );
            if (isBoundary) {
              canAccept = true;
              refreshBoundaryIDs.add(pendingID);
            }
          }
          if (canAccept) {
            return [];
          }
          const parentIDs = inverseDependencies[pendingID];
          if (parentIDs.length === 0) {
            performFullRefresh("No root boundary", {
              source: mod,
              failed: pendingModule,
            });
            didBailOut = true;
            return [];
          }
          return parentIDs;
        },
        () => didBailOut,
      ).reverse();
    } catch (e) {
      if (e === CYCLE_DETECTED) {
        performFullRefresh("Dependency cycle", {
          source: mod,
        });
        return;
      }
      throw e;
    }
    if (didBailOut) {
      return;
    }
    const seenModuleIDs = new Set();
    for (let i = 0; i < updatedModuleIDs.length; i++) {
      const updatedID = updatedModuleIDs[i];
      if (seenModuleIDs.has(updatedID)) {
        continue;
      }
      seenModuleIDs.add(updatedID);
      const updatedMod = modules.get(updatedID);
      if (updatedMod == null) {
        throw new Error("[Refresh] Expected to find the updated module.");
      }
      const prevExports = updatedMod.publicModule.exports;
      const didError = runUpdatedModule(
        updatedID,
        updatedID === id ? factory : undefined,
        updatedID === id ? dependencyMap : undefined,
      );
      const nextExports = updatedMod.publicModule.exports;
      if (didError) {
        return;
      }
      if (refreshBoundaryIDs.has(updatedID)) {
        const isNoLongerABoundary = !isReactRefreshBoundary(
          Refresh,
          nextExports,
        );
        const didInvalidate = shouldInvalidateReactRefreshBoundary(
          Refresh,
          prevExports,
          nextExports,
        );
        if (isNoLongerABoundary || didInvalidate) {
          const parentIDs = inverseDependencies[updatedID];
          if (parentIDs.length === 0) {
            performFullRefresh(
              isNoLongerABoundary
                ? "No longer a boundary"
                : "Invalidated boundary",
              {
                source: mod,
                failed: updatedMod,
              },
            );
            return;
          }
          for (let j = 0; j < parentIDs.length; j++) {
            const parentID = parentIDs[j];
            const parentMod = modules.get(parentID);
            if (parentMod == null) {
              throw new Error("[Refresh] Expected to find parent module.");
            }
            const canAcceptParent = isReactRefreshBoundary(
              Refresh,
              parentMod.publicModule.exports,
            );
            if (canAcceptParent) {
              refreshBoundaryIDs.add(parentID);
              updatedModuleIDs.push(parentID);
            } else {
              performFullRefresh("Invalidated boundary", {
                source: mod,
                failed: parentMod,
              });
              return;
            }
          }
        }
      }
    }
    if (Refresh != null) {
      if (reactRefreshTimeout == null) {
        reactRefreshTimeout = setTimeout(() => {
          reactRefreshTimeout = null;
          Refresh.performReactRefresh();
        }, 30);
      }
    }
  };
  const topologicalSort = function (roots, getEdges, earlyStop) {
    const result = [];
    const visited = new Set();
    const stack = new Set();
    function traverseDependentNodes(node) {
      if (stack.has(node)) {
        throw CYCLE_DETECTED;
      }
      if (visited.has(node)) {
        return;
      }
      visited.add(node);
      stack.add(node);
      const dependentNodes = getEdges(node);
      if (earlyStop(node)) {
        stack.delete(node);
        return;
      }
      dependentNodes.forEach((dependent) => {
        traverseDependentNodes(dependent);
      });
      stack.delete(node);
      result.push(node);
    }
    roots.forEach((root) => {
      traverseDependentNodes(root);
    });
    return result;
  };
  const runUpdatedModule = function (id, factory, dependencyMap) {
    const mod = modules.get(id);
    if (mod == null) {
      throw new Error("[Refresh] Expected to find the module.");
    }
    const { hot } = mod;
    if (!hot) {
      throw new Error("[Refresh] Expected module.hot to always exist in DEV.");
    }
    if (hot._disposeCallback) {
      try {
        hot._disposeCallback();
      } catch (error) {
        console.error(
          `Error while calling dispose handler for module ${id}: `,
          error,
        );
      }
    }
    if (factory) {
      mod.factory = factory;
    }
    if (dependencyMap) {
      mod.dependencyMap = dependencyMap;
    }
    mod.hasError = false;
    mod.error = undefined;
    mod.importedAll = EMPTY;
    mod.importedDefault = EMPTY;
    mod.isInitialized = false;
    const prevExports = mod.publicModule.exports;
    mod.publicModule.exports = {};
    hot._didAccept = false;
    hot._acceptCallback = null;
    hot._disposeCallback = null;
    metroRequire(id);
    if (mod.hasError) {
      mod.hasError = false;
      mod.isInitialized = true;
      mod.error = null;
      mod.publicModule.exports = prevExports;
      return true;
    }
    if (hot._acceptCallback) {
      try {
        hot._acceptCallback();
      } catch (error) {
        console.error(
          `Error while calling accept handler for module ${id}: `,
          error,
        );
      }
    }
    return false;
  };
  const performFullRefresh = (reason, modules) => {
    if (
      typeof window !== "undefined" &&
      window.location != null &&
      typeof window.location.reload === "function"
    ) {
      window.location.reload();
    } else {
      const Refresh = requireRefresh();
      if (Refresh != null) {
        const sourceName = modules.source?.verboseName ?? "unknown";
        const failedName = modules.failed?.verboseName ?? "unknown";
        Refresh.performFullRefresh(
          `Fast Refresh - ${reason} <${sourceName}> <${failedName}>`,
        );
      } else {
        console.warn("Could not reload the application after an edit.");
      }
    }
  };
  const isExportSafeToAccess = (moduleExports, key) => {
    return (
      moduleExports?.__esModule ||
      Object.getOwnPropertyDescriptor(moduleExports, key)?.get == null
    );
  };
  var isReactRefreshBoundary = function (Refresh, moduleExports) {
    if (Refresh.isLikelyComponentType(moduleExports)) {
      return true;
    }
    if (moduleExports == null || typeof moduleExports !== "object") {
      return false;
    }
    let hasExports = false;
    let areAllExportsComponents = true;
    for (const key in moduleExports) {
      hasExports = true;
      if (key === "__esModule") {
        continue;
      } else if (!isExportSafeToAccess(moduleExports, key)) {
        return false;
      }
      const exportValue = moduleExports[key];
      if (!Refresh.isLikelyComponentType(exportValue)) {
        areAllExportsComponents = false;
      }
    }
    return hasExports && areAllExportsComponents;
  };
  var shouldInvalidateReactRefreshBoundary = (
    Refresh,
    prevExports,
    nextExports,
  ) => {
    const prevSignature = getRefreshBoundarySignature(Refresh, prevExports);
    const nextSignature = getRefreshBoundarySignature(Refresh, nextExports);
    if (prevSignature.length !== nextSignature.length) {
      return true;
    }
    for (let i = 0; i < nextSignature.length; i++) {
      if (prevSignature[i] !== nextSignature[i]) {
        return true;
      }
    }
    return false;
  };
  var getRefreshBoundarySignature = (Refresh, moduleExports) => {
    const signature = [];
    signature.push(Refresh.getFamilyByType(moduleExports));
    if (moduleExports == null || typeof moduleExports !== "object") {
      return signature;
    }
    for (const key in moduleExports) {
      if (key === "__esModule") {
        continue;
      } else if (!isExportSafeToAccess(moduleExports, key)) {
        continue;
      }
      const exportValue = moduleExports[key];
      signature.push(key);
      signature.push(Refresh.getFamilyByType(exportValue));
    }
    return signature;
  };
  var registerExportsForReactRefresh = (Refresh, moduleExports, moduleID) => {
    Refresh.register(moduleExports, moduleID + " %exports%");
    if (moduleExports == null || typeof moduleExports !== "object") {
      return;
    }
    for (const key in moduleExports) {
      if (!isExportSafeToAccess(moduleExports, key)) {
        continue;
      }
      const exportValue = moduleExports[key];
      const typeID = moduleID + " %exports% " + key;
      Refresh.register(exportValue, typeID);
    }
  };
  global.__accept = metroHotUpdateModule;
}
if (__DEV__) {
  var requireSystrace = function requireSystrace() {
    return (
      global[__METRO_GLOBAL_PREFIX__ + "__SYSTRACE"] || metroRequire.Systrace
    );
  };
  var requireRefresh = function requireRefresh() {
    return (
      global[__METRO_GLOBAL_PREFIX__ + "__ReactRefresh"] ||
      global[global.__METRO_GLOBAL_PREFIX__ + "__ReactRefresh"] ||
      metroRequire.Refresh
    );
  };
}
