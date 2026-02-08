"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class FailedToResolveUnsupportedError extends Error {
  constructor(message) {
    super(message);
  }
}
exports.default = FailedToResolveUnsupportedError;
