"use strict";

var _utils = require("./utils");
const normalModule = require("./subdir-conflict");
const contextModule = require.context("./subdir-conflict");
function main() {
  return {
    normalModule,
    contextModule: (0, _utils.copyContextToObject)(contextModule),
  };
}
module.exports = main();
