#!/usr/bin/env node
"use strict";

try {
  require("metro-babel-register").unstable_registerForMetroMonorepo();
} catch {}
const { attachMetroCli } = require("./index");
const yargs = require("yargs");
attachMetroCli(yargs.demandCommand(1)).argv;
