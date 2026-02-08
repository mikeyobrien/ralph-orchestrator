"use strict";

const formatFileCandidates = require("./formatFileCandidates");
class InvalidPackageError extends Error {
  constructor(opts) {
    super(
      `The package \`${opts.packageJsonPath}\` is invalid because it ` +
        "specifies a `main` module field that could not be resolved (" +
        `\`${opts.mainModulePath}\`. None of these files exist:\n\n` +
        `  * ${formatFileCandidates(opts.fileCandidates)}\n` +
        `  * ${formatFileCandidates(opts.indexCandidates)}`
    );
    Object.assign(this, opts);
  }
}
module.exports = InvalidPackageError;
