"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = coerceKeyValueArray;
function coerceKeyValueArray(keyValueArray) {
  const result = Object.create(null);
  for (const item of keyValueArray) {
    if (item.indexOf("=") === -1) {
      throw new Error('Expected parameter to include "=" but found: ' + item);
    }
    if (item.indexOf("&") !== -1) {
      throw new Error('Parameter cannot include "&" but found: ' + item);
    }
    const params = new URLSearchParams(item);
    params.forEach((value, key) => {
      result[key] = value;
    });
  }
  return result;
}
