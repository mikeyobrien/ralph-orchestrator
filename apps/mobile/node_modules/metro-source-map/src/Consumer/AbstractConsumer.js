"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _constants = require("./constants");
var _invariant = _interopRequireDefault(require("invariant"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class AbstractConsumer {
  constructor(sourceMap) {
    this._sourceMap = sourceMap;
  }
  originalPositionFor(generatedPosition) {
    (0, _invariant.default)(false, "Not implemented");
  }
  generatedMappings() {
    (0, _invariant.default)(false, "Not implemented");
  }
  eachMapping(callback, context = null, order = _constants.GENERATED_ORDER) {
    (0, _invariant.default)(
      order === _constants.GENERATED_ORDER,
      `Iteration order not implemented: ${(0, _constants.iterationOrderToString)(order)}`,
    );
    for (const mapping of this.generatedMappings()) {
      callback.call(context, mapping);
    }
  }
  get file() {
    return this._sourceMap.file;
  }
  sourceContentFor(source, nullOnMissing) {
    (0, _invariant.default)(false, "Not implemented");
  }
}
exports.default = AbstractConsumer;
