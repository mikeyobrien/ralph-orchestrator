"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = writeSourcemap;
var _writeFile = _interopRequireDefault(require("../writeFile"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function writeSourcemap(fileName, contents, log) {
  if (!fileName) {
    return Promise.resolve();
  }
  log("Writing sourcemap output to:", fileName);
  const writeMap = (0, _writeFile.default)(fileName, contents);
  writeMap.then(() => log("Done writing sourcemap output"));
  return writeMap;
}
