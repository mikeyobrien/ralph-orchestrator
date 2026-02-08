"use strict";

async function main() {
  return {
    counterModuleId1: require.resolveWeak("./subdir/counter-module"),
    counterModuleId2: require.resolveWeak("./subdir/counter-module.js"),
    throwingModuleId: require.resolveWeak("./subdir/throwing-module.js"),
  };
}
module.exports = main();
