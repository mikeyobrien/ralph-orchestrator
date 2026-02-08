"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class RevisionNotFoundError extends Error {
  constructor(revisionId) {
    super(`The revision \`${revisionId}\` was not found.`);
    this.revisionId = revisionId;
  }
}
exports.default = RevisionNotFoundError;
