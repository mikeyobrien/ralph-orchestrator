"use strict";

var path = require("path");
var list = [/\/__tests__\/.*/];
function escapeRegExp(pattern) {
  if (Object.prototype.toString.call(pattern) === "[object RegExp]") {
    return pattern.source.replace(/\/|\\\//g, "\\" + path.sep);
  } else if (typeof pattern === "string") {
    var escaped = pattern.replace(/[\-\[\]\{\}\(\)\*\+\?\.\\\^\$\|]/g, "\\$&");
    return escaped.replaceAll("/", "\\" + path.sep);
  } else {
    throw new Error("Unexpected exclusion pattern: " + pattern);
  }
}
function exclusionList(additionalExclusions) {
  return new RegExp(
    "(" +
      (additionalExclusions || []).concat(list).map(escapeRegExp).join("|") +
      ")$"
  );
}
module.exports = exclusionList;
