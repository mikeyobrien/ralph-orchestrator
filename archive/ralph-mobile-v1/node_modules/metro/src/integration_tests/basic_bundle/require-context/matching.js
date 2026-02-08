"use strict";

const ab = require.context("./subdir", false, /\/(a|b)\.js$/);
const abc = require.context("./subdir", false);
const abcd = require.context("./subdir", true);
function main() {
  return {
    ab: ab.keys(),
    abc: abc.keys(),
    abcd: abcd.keys(),
  };
}
module.exports = main();
