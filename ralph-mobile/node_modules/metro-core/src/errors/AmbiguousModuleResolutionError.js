"use strict";

class AmbiguousModuleResolutionError extends Error {
  constructor(fromModulePath, hasteError) {
    super(
      `Ambiguous module resolution from \`${fromModulePath}\`: ` +
        hasteError.message
    );
    this.fromModulePath = fromModulePath;
    this.hasteError = hasteError;
  }
}
module.exports = AmbiguousModuleResolutionError;
