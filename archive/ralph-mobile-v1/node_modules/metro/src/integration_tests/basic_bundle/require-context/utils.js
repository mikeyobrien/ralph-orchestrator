"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.awaitProperties = awaitProperties;
exports.copyContextToObject = copyContextToObject;
function copyContextToObject(ctx) {
  return Object.fromEntries(ctx.keys().map((key) => [key, ctx(key)]));
}
function awaitProperties(obj) {
  const result = {};
  return Promise.all(
    Object.keys(obj).map((key) => {
      return obj[key].then((value) => (result[key] = value));
    }),
  ).then(() => result);
}
