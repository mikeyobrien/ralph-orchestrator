"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class AmbiguousModuleResolutionError extends Error {
  constructor(fromModulePath, hasteError) {
    super(
      `Ambiguous module resolution from \`${fromModulePath}\`: ` +
        hasteError.message,
    );
    this.fromModulePath = fromModulePath;
    this.hasteError = hasteError;
  }
}
exports.default = AmbiguousModuleResolutionError;
