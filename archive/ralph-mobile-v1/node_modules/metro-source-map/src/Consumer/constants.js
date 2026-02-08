"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.ORIGINAL_ORDER =
  exports.LEAST_UPPER_BOUND =
  exports.GREATEST_LOWER_BOUND =
  exports.GENERATED_ORDER =
  exports.FIRST_LINE =
  exports.FIRST_COLUMN =
  exports.EMPTY_POSITION =
    void 0;
exports.iterationOrderToString = iterationOrderToString;
exports.lookupBiasToString = lookupBiasToString;
var _ob = require("ob1");
const FIRST_COLUMN = (exports.FIRST_COLUMN = (0, _ob.add0)(0));
const FIRST_LINE = (exports.FIRST_LINE = (0, _ob.add1)(0));
const GENERATED_ORDER = (exports.GENERATED_ORDER = "GENERATED_ORDER");
const ORIGINAL_ORDER = (exports.ORIGINAL_ORDER = "ORIGINAL_ORDER");
const GREATEST_LOWER_BOUND = (exports.GREATEST_LOWER_BOUND =
  "GREATEST_LOWER_BOUND");
const LEAST_UPPER_BOUND = (exports.LEAST_UPPER_BOUND = "LEAST_UPPER_BOUND");
const EMPTY_POSITION = (exports.EMPTY_POSITION = Object.freeze({
  source: null,
  name: null,
  line: null,
  column: null,
}));
function iterationOrderToString(x) {
  return x;
}
function lookupBiasToString(x) {
  return x;
}
