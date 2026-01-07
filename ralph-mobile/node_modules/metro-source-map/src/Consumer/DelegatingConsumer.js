"use strict";

const {
  GENERATED_ORDER,
  GREATEST_LOWER_BOUND,
  LEAST_UPPER_BOUND,
  ORIGINAL_ORDER,
} = require("./constants");
const createConsumer = require("./createConsumer");
class DelegatingConsumer {
  static GENERATED_ORDER = GENERATED_ORDER;
  static ORIGINAL_ORDER = ORIGINAL_ORDER;
  static GREATEST_LOWER_BOUND = GREATEST_LOWER_BOUND;
  static LEAST_UPPER_BOUND = LEAST_UPPER_BOUND;
  constructor(sourceMap) {
    this._rootConsumer = createConsumer(sourceMap);
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
module.exports = DelegatingConsumer;
