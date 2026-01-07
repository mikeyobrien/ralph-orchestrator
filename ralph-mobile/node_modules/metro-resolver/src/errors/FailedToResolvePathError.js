"use strict";

const formatFileCandidates = require("./formatFileCandidates");
class FailedToResolvePathError extends Error {
  constructor(candidates) {
    super(
      "The module could not be resolved because none of these files exist:\n\n" +
        [candidates.file, candidates.dir]
          .filter(Boolean)
          .map((candidates) => `  * ${formatFileCandidates(candidates)}`)
          .join("\n")
    );
    this.candidates = candidates;
  }
}
module.exports = FailedToResolvePathError;
