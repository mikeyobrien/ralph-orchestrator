"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.isSubpathDefinedInExportsLike = isSubpathDefinedInExportsLike;
var _matchSubpathPattern = require("./matchSubpathPattern");
function isSubpathDefinedInExportsLike(exportsLikeMap, subpath) {
  if (exportsLikeMap.has(subpath)) {
    return true;
  }
  for (const key of exportsLikeMap.keys()) {
    if (
      key.split("*").length === 2 &&
      (0, _matchSubpathPattern.matchSubpathPattern)(key, subpath) != null
    ) {
      return true;
    }
  }
  return false;
}
