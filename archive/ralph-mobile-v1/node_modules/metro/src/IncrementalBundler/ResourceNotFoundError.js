"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class ResourceNotFoundError extends Error {
  constructor(resourcePath) {
    super(`The resource \`${resourcePath}\` was not found.`);
    this.resourcePath = resourcePath;
  }
}
exports.default = ResourceNotFoundError;
