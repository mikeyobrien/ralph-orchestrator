"use strict";

const empty = require.context("./no-such-dir");
function main() {
  try {
    empty("./no-such-file.js");
  } catch (e) {
    return {
      error: {
        message: e.message,
        code: e.code,
      },
    };
  }
  return null;
}
module.exports = main();
