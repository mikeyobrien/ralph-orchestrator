"use strict";

const fs = require("fs");
exports.watchFile = async function (filename, callback) {
  fs.watchFile(filename, () => {
    callback();
  });
  await callback();
};
exports.makeAsyncCommand = (command) => (argv) => {
  Promise.resolve(command(argv)).catch((error) => {
    console.error(error.stack);
    process.exitCode = 1;
  });
};
