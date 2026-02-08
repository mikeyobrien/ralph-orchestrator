"use strict";

const key = `${global.__METRO_GLOBAL_PREFIX__ ?? ""}__loadBundleAsync`;
global[key] = async function loadBundleAsyncForTest(path) {
  await __DOWNLOAD_AND_EXEC_FOR_TESTS__(path);
};
