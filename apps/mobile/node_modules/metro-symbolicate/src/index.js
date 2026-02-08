#!/usr/bin/env node
"use strict";

var _symbolicate = _interopRequireDefault(require("./symbolicate"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
(0, _symbolicate.default)().then((code) => process.exit(code));
