"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
var _RamBundle = require("./RamBundle");
Object.keys(_RamBundle).forEach(function (key) {
  if (key === "default" || key === "__esModule") return;
  if (key in exports && exports[key] === _RamBundle[key]) return;
  Object.defineProperty(exports, key, {
    enumerable: true,
    get: function () {
      return _RamBundle[key];
    },
  });
});
