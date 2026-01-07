"use strict";

class ResourceNotFoundError extends Error {
  constructor(resourcePath) {
    super(`The resource \`${resourcePath}\` was not found.`);
    this.resourcePath = resourcePath;
  }
}
module.exports = ResourceNotFoundError;
