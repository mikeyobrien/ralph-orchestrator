"use strict";

const { GENERATED_ORDER, iterationOrderToString } = require("./constants");
const invariant = require("invariant");
class AbstractConsumer {
  constructor(sourceMap) {
    this._sourceMap = sourceMap;
  }
  originalPositionFor(generatedPosition) {
    invariant(false, "Not implemented");
  }
  generatedMappings() {
    invariant(false, "Not implemented");
  }
  eachMapping(callback, context = null, order = GENERATED_ORDER) {
    invariant(
      order === GENERATED_ORDER,
      `Iteration order not implemented: ${iterationOrderToString(order)}`
    );
    for (const mapping of this.generatedMappings()) {
      callback.call(context, mapping);
    }
  }
  get file() {
    return this._sourceMap.file;
  }
  sourceContentFor(source, nullOnMissing) {
    invariant(false, "Not implemented");
  }
}
module.exports = AbstractConsumer;
