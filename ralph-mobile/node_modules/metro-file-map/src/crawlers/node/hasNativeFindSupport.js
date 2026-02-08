"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = hasNativeFindSupport;
var _child_process = require("child_process");
async function hasNativeFindSupport() {
  try {
    return await new Promise((resolve) => {
      const args = [
        ".",
        "-type",
        "f",
        "(",
        "-iname",
        "*.ts",
        "-o",
        "-iname",
        "*.js",
        ")",
      ];
      const child = (0, _child_process.spawn)("find", args, {
        cwd: __dirname,
      });
      child.on("error", () => {
        resolve(false);
      });
      child.on("exit", (code) => {
        resolve(code === 0);
      });
    });
  } catch {
    return false;
  }
}
