"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class GraphNotFoundError extends Error {
  constructor(graphId) {
    super(`The graph \`${graphId}\` was not found.`);
    this.graphId = graphId;
  }
}
exports.default = GraphNotFoundError;
