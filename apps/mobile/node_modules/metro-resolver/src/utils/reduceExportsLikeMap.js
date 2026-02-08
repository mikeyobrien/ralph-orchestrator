"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.reduceExportsLikeMap = reduceExportsLikeMap;
function reduceExportsLikeMap(
  exportsLikeMap,
  conditionNames,
  createConfigError,
) {
  const result = new Map();
  for (const [subpath, value] of exportsLikeMap) {
    const subpathValue = reduceConditionalExport(value, conditionNames);
    if (subpathValue !== "no-match") {
      result.set(subpath, subpathValue);
    }
  }
  for (const value of result.values()) {
    if (value != null && !value.startsWith("./")) {
      throw createConfigError(
        'One or more mappings for subpaths defined in "exports" are invalid. ' +
          'All values must begin with "./".',
      );
    }
  }
  return result;
}
function reduceConditionalExport(subpathValue, conditionNames) {
  let reducedValue = subpathValue;
  while (reducedValue != null && typeof reducedValue !== "string") {
    let match;
    if ("default" in reducedValue) {
      match = "no-match";
    } else {
      match = null;
    }
    for (const conditionName in reducedValue) {
      if (conditionNames.has(conditionName)) {
        match = reducedValue[conditionName];
        break;
      }
    }
    reducedValue = match;
  }
  return reducedValue;
}
