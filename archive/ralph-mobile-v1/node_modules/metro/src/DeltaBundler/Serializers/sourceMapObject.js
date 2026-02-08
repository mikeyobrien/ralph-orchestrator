"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.sourceMapObject = sourceMapObject;
exports.sourceMapObjectNonBlocking = sourceMapObjectNonBlocking;
var _sourceMapGenerator = require("./sourceMapGenerator");
function sourceMapObject(modules, options) {
  const generator = (0, _sourceMapGenerator.sourceMapGenerator)(
    modules,
    options,
  );
  return generator.toMap(undefined, {
    excludeSource: options.excludeSource,
  });
}
async function sourceMapObjectNonBlocking(modules, options) {
  const generator = await (0,
  _sourceMapGenerator.sourceMapGeneratorNonBlocking)(modules, options);
  return generator.toMap(undefined, {
    excludeSource: options.excludeSource,
  });
}
