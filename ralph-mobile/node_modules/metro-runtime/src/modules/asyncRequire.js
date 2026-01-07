"use strict";

function maybeLoadBundle(moduleID, paths) {
  const loadBundle = global[`${__METRO_GLOBAL_PREFIX__}__loadBundleAsync`];
  if (loadBundle != null) {
    const stringModuleID = String(moduleID);
    if (paths != null) {
      const bundlePath = paths[stringModuleID];
      if (bundlePath != null) {
        return loadBundle(bundlePath);
      }
    }
  }
  return undefined;
}
function asyncRequireImpl(moduleID, paths) {
  const maybeLoadBundlePromise = maybeLoadBundle(moduleID, paths);
  const importAll = () => require.importAll(moduleID);
  if (maybeLoadBundlePromise != null) {
    return maybeLoadBundlePromise.then(importAll);
  }
  return importAll();
}
async function asyncRequire(moduleID, paths, moduleName) {
  return asyncRequireImpl(moduleID, paths);
}
asyncRequire.unstable_importMaybeSync = function unstable_importMaybeSync(
  moduleID,
  paths
) {
  return asyncRequireImpl(moduleID, paths);
};
asyncRequire.prefetch = function (moduleID, paths, moduleName) {
  maybeLoadBundle(moduleID, paths)?.then(
    () => {},
    () => {}
  );
};
module.exports = asyncRequire;
