"use strict";

function main() {
  const moduleId = require.resolveWeak("./subdir/counter-module");
  const dynamicRequire = require;
  dynamicRequire(moduleId).increment();
  const timesIncremented = require("./subdir/counter-module.js").increment();
  return {
    moduleId,
    timesIncremented,
  };
}
module.exports = main();
