"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.TOUCH_EVENT = exports.DELETE_EVENT = exports.ALL_EVENT = void 0;
exports.includedByGlob = includedByGlob;
exports.posixPathMatchesPattern = void 0;
exports.typeFromStat = typeFromStat;
const micromatch = require("micromatch");
const path = require("path");
const DELETE_EVENT = (exports.DELETE_EVENT = "delete");
const TOUCH_EVENT = (exports.TOUCH_EVENT = "touch");
const ALL_EVENT = (exports.ALL_EVENT = "all");
function includedByGlob(type, globs, dot, relativePath) {
  if (globs.length === 0 || type !== "f") {
    return dot || micromatch.some(relativePath, "**/*");
  }
  return micromatch.some(relativePath, globs, {
    dot,
  });
}
const posixPathMatchesPattern = (exports.posixPathMatchesPattern =
  path.sep === "/"
    ? (pattern, filePath) => pattern.test(filePath)
    : (pattern, filePath) => pattern.test(filePath.replaceAll(path.sep, "/")));
function typeFromStat(stat) {
  if (stat.isSymbolicLink()) {
    return "l";
  }
  if (stat.isDirectory()) {
    return "d";
  }
  if (stat.isFile()) {
    return "f";
  }
  return null;
}
