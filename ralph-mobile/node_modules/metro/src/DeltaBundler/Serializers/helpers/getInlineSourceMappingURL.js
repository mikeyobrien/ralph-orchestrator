"use strict";

function getInlineSourceMappingURL(sourceMap) {
  const base64 = Buffer.from(sourceMap).toString("base64");
  return `data:application/json;charset=utf-8;base64,${base64}`;
}
module.exports = getInlineSourceMappingURL;
