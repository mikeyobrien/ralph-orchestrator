"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _metroResolver = require("metro-resolver");
class PackageResolutionError extends Error {
  constructor(opts) {
    const perr = opts.packageError;
    super(
      `While trying to resolve module \`${opts.targetModuleName}\` from file ` +
        `\`${opts.originModulePath}\`, the package ` +
        `\`${perr.packageJsonPath}\` was successfully found. However, ` +
        "this package itself specifies " +
        "a `main` module field that could not be resolved (" +
        `\`${perr.mainModulePath}\`. Indeed, none of these files exist:\n\n` +
        `  * ${(0, _metroResolver.formatFileCandidates)(perr.fileCandidates)}\n` +
        `  * ${(0, _metroResolver.formatFileCandidates)(perr.indexCandidates)}`,
    );
    Object.assign(this, opts);
  }
}
exports.default = PackageResolutionError;
