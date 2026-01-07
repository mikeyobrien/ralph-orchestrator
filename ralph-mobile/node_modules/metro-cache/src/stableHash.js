"use strict";

const crypto = require("crypto");
const canonicalize = require("metro-core/src/canonicalize");
function stableHash(value) {
  return crypto
    .createHash("md5")
    .update(JSON.stringify(value, canonicalize))
    .digest("buffer");
}
module.exports = stableHash;
