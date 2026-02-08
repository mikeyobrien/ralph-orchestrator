"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.c = exports.b = exports.a = void 0;
let shouldBeB, shouldBeC;
try {
  shouldBeB = require("./not-exists");
} catch {
  shouldBeB = require("./optional-b");
}
(function requireOptionalC() {
  try {
    shouldBeC = require("./optional-c");
  } catch (e) {}
})();
const a = (exports.a = require("./required-a"));
const b = (exports.b = shouldBeB);
const c = (exports.c = shouldBeC);
