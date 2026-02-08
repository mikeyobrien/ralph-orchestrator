"use strict";

var _utils = require("./utils");
function main() {
  return (0, _utils.awaitProperties)(
    (0, _utils.copyContextToObject)(
      require.context("./subdir", undefined, undefined, "lazy")
    )
  );
}
module.exports = main();
