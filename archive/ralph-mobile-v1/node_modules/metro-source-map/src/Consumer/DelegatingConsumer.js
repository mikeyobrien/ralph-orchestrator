"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _constants = require("./constants");
var _createConsumer = _interopRequireDefault(require("./createConsumer"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class DelegatingConsumer {
  static GENERATED_ORDER = _constants.GENERATED_ORDER;
  static ORIGINAL_ORDER = _constants.ORIGINAL_ORDER;
  static GREATEST_LOWER_BOUND = _constants.GREATEST_LOWER_BOUND;
  static LEAST_UPPER_BOUND = _constants.LEAST_UPPER_BOUND;
  constructor(sourceMap) {
    this._rootConsumer = (0, _createConsumer.default)(sourceMap);
    return this._rootConsumer;
  }
  originalPositionFor(generatedPosition) {
    return this._rootConsumer.originalPositionFor(generatedPosition);
  }
  generatedMappings() {
    return this._rootConsumer.generatedMappings();
  }
  eachMapping(callback, context, order) {
    return this._rootConsumer.eachMapping(callback, context, order);
  }
  get file() {
    return this._rootConsumer.file;
  }
  sourceContentFor(source, nullOnMissing) {
    return this._rootConsumer.sourceContentFor(source, nullOnMissing);
  }
}
exports.default = DelegatingConsumer;
