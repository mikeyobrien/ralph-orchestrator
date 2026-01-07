"use strict";

class NetworkError extends Error {
  constructor(message, code) {
    super(message);
    this.code = code;
  }
}
module.exports = NetworkError;
