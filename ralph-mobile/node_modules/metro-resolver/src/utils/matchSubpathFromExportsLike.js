"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.matchSubpathFromExportsLike = matchSubpathFromExportsLike;
var _matchSubpathPattern = require("./matchSubpathPattern");
var _reduceExportsLikeMap = require("./reduceExportsLikeMap");
function matchSubpathFromExportsLike(
  context,
  subpath,
  exportsLikeMap,
  platform,
  createConfigError
) {
  const conditionNames = new Set([
    "default",
    context.isESMImport === true ? "import" : "require",
    ...context.unstable_conditionNames,
    ...(platform != null
      ? context.unstable_conditionsByPlatform[platform] ?? []
      : []),
  ]);
  const exportsLikeMapAfterConditions = (0,
  _reduceExportsLikeMap.reduceExportsLikeMap)(
    exportsLikeMap,
    conditionNames,
    createConfigError
  );
  let target = exportsLikeMapAfterConditions.get(subpath);
  let patternMatch = null;
  if (target == null) {
    const expansionKeys = [...exportsLikeMapAfterConditions.keys()]
      .map((key) => ({
        key,
        baseLength: key.indexOf("*"),
      }))
      .filter((data) => data.baseLength !== -1)
      .sort((a, b) => {
        if (a.baseLength === b.baseLength) {
          return b.key.length - a.key.length;
        }
        return b.baseLength - a.baseLength;
      });
    for (const { key } of expansionKeys) {
      const value = exportsLikeMapAfterConditions.get(key);
      patternMatch = (0, _matchSubpathPattern.matchSubpathPattern)(
        key,
        subpath
      );
      if (patternMatch != null) {
        target = value;
        break;
      }
    }
  }
  return {
    target: target ?? null,
    patternMatch,
  };
}
