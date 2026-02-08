"use strict";

const vm = require("vm");
module.exports = function execBundle(code, context = {}) {
  if (vm.isContext(context)) {
    return vm.runInContext(code, context);
  }
  return vm.runInNewContext(code, context);
};
