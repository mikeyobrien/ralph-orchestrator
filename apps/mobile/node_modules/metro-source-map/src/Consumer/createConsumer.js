"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = createConsumer;
var _MappingsConsumer = _interopRequireDefault(require("./MappingsConsumer"));
var _SectionsConsumer = _interopRequireDefault(require("./SectionsConsumer"));
var _invariant = _interopRequireDefault(require("invariant"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function createConsumer(sourceMap) {
  (0, _invariant.default)(
    sourceMap.version === "3" || sourceMap.version === 3,
    `Unrecognized source map format version: ${sourceMap.version}`,
  );
  if (sourceMap.mappings === undefined) {
    return new _SectionsConsumer.default(sourceMap);
  }
  return new _MappingsConsumer.default(sourceMap);
}
