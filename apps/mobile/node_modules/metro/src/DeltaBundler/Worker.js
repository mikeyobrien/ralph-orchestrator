"use strict";

try {
  require("metro-babel-register").unstable_registerForMetroMonorepo();
} catch {}
module.exports = require("./Worker.flow");
