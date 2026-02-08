"use strict";

const DoesNotExist = require("./foo");
global.x = DoesNotExist;
