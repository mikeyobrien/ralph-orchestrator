"use strict";

const fs = require("fs");
module.exports = {
  getHasteName(filename) {
    const matches = fs
      .readFileSync(filename, "utf8")
      .match(/@providesModule ([^\n]+)/);
    if (!matches) {
      return undefined;
    }
    return matches[1];
  },
  getCacheKey() {
    return "hasteImplFixture";
  },
};
