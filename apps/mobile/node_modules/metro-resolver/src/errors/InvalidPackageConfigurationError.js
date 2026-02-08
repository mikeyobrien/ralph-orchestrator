"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class InvalidPackageConfigurationError extends Error {
  constructor(opts) {
    super(
      `The package ${opts.packagePath} contains an invalid package.json ` +
        "configuration. Consider raising this issue with the package " +
        "maintainer(s).\nReason: " +
        opts.reason,
    );
    Object.assign(this, opts);
  }
}
exports.default = InvalidPackageConfigurationError;
