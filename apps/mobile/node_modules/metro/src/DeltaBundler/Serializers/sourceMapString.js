"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.sourceMapString = sourceMapString;
exports.sourceMapStringNonBlocking = sourceMapStringNonBlocking;
var _sourceMapGenerator = require("./sourceMapGenerator");
function sourceMapString(modules, options) {
  return (0, _sourceMapGenerator.sourceMapGenerator)(modules, options).toString(
    undefined,
    {
      excludeSource: options.excludeSource,
    },
  );
}
async function sourceMapStringNonBlocking(modules, options) {
  const generator = await (0,
  _sourceMapGenerator.sourceMapGeneratorNonBlocking)(modules, options);
  return generator.toString(undefined, {
    excludeSource: options.excludeSource,
  });
}
