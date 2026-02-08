"use strict";

const {
  sourceMapGenerator,
  sourceMapGeneratorNonBlocking,
} = require("./sourceMapGenerator");
function sourceMapString(modules, options) {
  return sourceMapGenerator(modules, options).toString(undefined, {
    excludeSource: options.excludeSource,
  });
}
async function sourceMapStringNonBlocking(modules, options) {
  const generator = await sourceMapGeneratorNonBlocking(modules, options);
  return generator.toString(undefined, {
    excludeSource: options.excludeSource,
  });
}
module.exports = {
  sourceMapString,
  sourceMapStringNonBlocking,
};
