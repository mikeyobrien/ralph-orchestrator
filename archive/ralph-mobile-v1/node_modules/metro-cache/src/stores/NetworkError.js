"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class NetworkError extends Error {
  constructor(message, code) {
    super(message);
    this.code = code;
  }
}
exports.default = NetworkError;
