"use strict";

const { add0, add1 } = require("ob1");
const FIRST_COLUMN = add0(0);
const FIRST_LINE = add1(0);
const GENERATED_ORDER = "GENERATED_ORDER";
const ORIGINAL_ORDER = "ORIGINAL_ORDER";
const GREATEST_LOWER_BOUND = "GREATEST_LOWER_BOUND";
const LEAST_UPPER_BOUND = "LEAST_UPPER_BOUND";
const EMPTY_POSITION = Object.freeze({
  source: null,
  name: null,
  line: null,
  column: null,
});
function iterationOrderToString(x) {
  return x;
}
function lookupBiasToString(x) {
  return x;
}
module.exports = {
  FIRST_COLUMN,
  FIRST_LINE,
  GENERATED_ORDER,
  ORIGINAL_ORDER,
  GREATEST_LOWER_BOUND,
  LEAST_UPPER_BOUND,
  EMPTY_POSITION,
  iterationOrderToString,
  lookupBiasToString,
};
