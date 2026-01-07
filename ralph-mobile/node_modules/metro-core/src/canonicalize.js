"use strict";

function canonicalize(key, value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return value;
  }
  const keys = Object.keys(value).sort();
  const length = keys.length;
  const object = {};
  for (let i = 0; i < length; i++) {
    object[keys[i]] = value[keys[i]];
  }
  return object;
}
module.exports = canonicalize;
