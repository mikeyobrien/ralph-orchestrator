"use strict";

async function main() {
  const moduleId = require.resolveWeak("./subdir/counter-module");
  (await import("./subdir/counter-module.js")).increment();
  const dynamicRequire = require;
  const timesIncremented = dynamicRequire(moduleId).increment();
  return {
    moduleId,
    timesIncremented,
  };
}
module.exports = main();
