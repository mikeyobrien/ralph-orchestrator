"use strict";

const {
  sourceMapGenerator,
  sourceMapGeneratorNonBlocking,
} = require("./sourceMapGenerator");
function sourceMapObject(modules, options) {
  const generator = sourceMapGenerator(modules, options);
  return generator.toMap(undefined, {
    excludeSource: options.excludeSource,
  });
}
async function sourceMapObjectNonBlocking(modules, options) {
  const generator = await sourceMapGeneratorNonBlocking(modules, options);
  return generator.toMap(undefined, {
    excludeSource: options.excludeSource,
  });
}
module.exports = {
  sourceMapObject,
  sourceMapObjectNonBlocking,
};
