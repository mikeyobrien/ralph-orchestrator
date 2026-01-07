"use strict";

class RevisionNotFoundError extends Error {
  constructor(revisionId) {
    super(`The revision \`${revisionId}\` was not found.`);
    this.revisionId = revisionId;
  }
}
module.exports = RevisionNotFoundError;
