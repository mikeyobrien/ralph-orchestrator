"use strict";

const util = require("source-map/lib/util");
function normalizeSourcePath(sourceInput, map) {
  const { sourceRoot } = map;
  let source = sourceInput;
  source = String(source);
  source = util.normalize(source);
  source =
    sourceRoot != null && util.isAbsolute(sourceRoot) && util.isAbsolute(source)
      ? util.relative(sourceRoot, source)
      : source;
  return source;
}
module.exports = normalizeSourcePath;
