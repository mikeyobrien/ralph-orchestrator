"use strict";

class GraphNotFoundError extends Error {
  constructor(graphId) {
    super(`The graph \`${graphId}\` was not found.`);
    this.graphId = graphId;
  }
}
module.exports = GraphNotFoundError;
