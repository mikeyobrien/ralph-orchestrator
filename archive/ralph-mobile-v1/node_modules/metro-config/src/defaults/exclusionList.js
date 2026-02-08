"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = exclusionList;
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const list = [/\/__tests__\/.*/];
function escapeRegExp(pattern) {
  if (pattern instanceof RegExp) {
    return pattern.source.replace(/\/|\\\//g, "\\" + _path.default.sep);
  } else if (typeof pattern === "string") {
    const escaped = pattern.replace(
      /[\-\[\]\{\}\(\)\*\+\?\.\\\^\$\|]/g,
      "\\$&",
    );
    return escaped.replaceAll("/", "\\" + _path.default.sep);
  } else {
    throw new Error(
      `Expected exclusionList to be called with RegExp or string, got: ${typeof pattern}`,
    );
  }
}
function exclusionList(additionalExclusions) {
  return new RegExp(
    "(" +
      (additionalExclusions || []).concat(list).map(escapeRegExp).join("|") +
      ")$",
  );
}
