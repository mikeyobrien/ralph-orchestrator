#!/usr/bin/env node
"use strict";

require("./symbolicate.js")().then((code) => process.exit(code));
