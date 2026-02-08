"use strict";

const crypto = require("crypto");
const fs = require("fs");
function getCacheKey(files) {
  return files
    .reduce(
      (hash, file) => hash.update("\0", "utf8").update(fs.readFileSync(file)),
      crypto.createHash("md5")
    )
    .digest("hex");
}
module.exports = getCacheKey;
