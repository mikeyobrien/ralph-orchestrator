"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class PackageImportNotResolvedError extends Error {
  constructor(opts) {
    super(
      `The path for ${opts.importSpecifier} could not be resolved.\nReason: ` +
        opts.reason,
    );
    this.importSpecifier = opts.importSpecifier;
    this.reason = opts.reason;
  }
}
exports.default = PackageImportNotResolvedError;
