"use strict";

const invariant = require("invariant");
function createConsumer(sourceMap) {
  invariant(
    sourceMap.version === "3" || sourceMap.version === 3,
    `Unrecognized source map format version: ${sourceMap.version}`
  );
  const MappingsConsumer = require("./MappingsConsumer");
  const SectionsConsumer = require("./SectionsConsumer");
  if (sourceMap.mappings === undefined) {
    return new SectionsConsumer(sourceMap);
  }
  return new MappingsConsumer(sourceMap);
}
module.exports = createConsumer;
