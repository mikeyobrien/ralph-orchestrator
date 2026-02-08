"use strict";

class HttpError extends Error {
  constructor(message, code) {
    super(message);
    this.code = code;
  }
}
module.exports = HttpError;
