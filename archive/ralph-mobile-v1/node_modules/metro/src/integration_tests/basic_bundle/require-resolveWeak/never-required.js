"use strict";

function main() {
  return {
    moduleId: require.resolveWeak("./subdir/throwing-module"),
  };
}
module.exports = main();
