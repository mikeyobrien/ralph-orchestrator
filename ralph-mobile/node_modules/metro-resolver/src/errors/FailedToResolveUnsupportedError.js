"use strict";

class FailedToResolveUnsupportedError extends Error {
  constructor(message) {
    super(message);
  }
}
module.exports = FailedToResolveUnsupportedError;
