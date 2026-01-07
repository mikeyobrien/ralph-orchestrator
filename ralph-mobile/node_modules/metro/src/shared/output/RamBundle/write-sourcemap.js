"use strict";

const writeFile = require("../writeFile");
function writeSourcemap(fileName, contents, log) {
  if (!fileName) {
    return Promise.resolve();
  }
  log("Writing sourcemap output to:", fileName);
  const writeMap = writeFile(fileName, contents);
  writeMap.then(() => log("Done writing sourcemap output"));
  return writeMap;
}
module.exports = writeSourcemap;
