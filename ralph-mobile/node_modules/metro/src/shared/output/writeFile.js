"use strict";

const fs = require("fs");
const throat = require("throat");
const writeFile = throat(128, fs.promises.writeFile);
module.exports = writeFile;
