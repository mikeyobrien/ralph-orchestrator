"use strict";

var _vm = _interopRequireDefault(require("vm"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
module.exports = function execBundle(code, context = {}) {
  if (_vm.default.isContext(context)) {
    return _vm.default.runInContext(code, context);
  }
  return _vm.default.runInNewContext(code, context);
};
