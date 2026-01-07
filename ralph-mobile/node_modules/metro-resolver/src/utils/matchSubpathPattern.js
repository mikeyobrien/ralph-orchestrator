"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.matchSubpathPattern = matchSubpathPattern;
function matchSubpathPattern(subpathPattern, subpath) {
  const [patternBase, patternTrailer] = subpathPattern.split("*");
  if (
    subpath.startsWith(patternBase) &&
    (patternTrailer.length === 0 ||
      (subpath.endsWith(patternTrailer) &&
        subpath.length >= subpathPattern.length))
  ) {
    return subpath.substring(
      patternBase.length,
      subpath.length - patternTrailer.length
    );
  }
  return null;
}
