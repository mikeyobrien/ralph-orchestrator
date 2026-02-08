"use strict";

const DeltaCalculator = require("./DeltaBundler/DeltaCalculator");
class DeltaBundler {
  _deltaCalculators = new Map();
  constructor(changeEventSource) {
    this._changeEventSource = changeEventSource;
  }
  end() {
    this._deltaCalculators.forEach((deltaCalculator) => deltaCalculator.end());
    this._deltaCalculators = new Map();
  }
  async getDependencies(entryPoints, options) {
    const deltaCalculator = new DeltaCalculator(
      new Set(entryPoints),
      this._changeEventSource,
      options
    );
    await deltaCalculator.getDelta({
      reset: true,
      shallow: options.shallow,
    });
    const graph = deltaCalculator.getGraph();
    deltaCalculator.end();
    return graph.dependencies;
  }
  async buildGraph(entryPoints, options) {
    const deltaCalculator = new DeltaCalculator(
      new Set(entryPoints),
      this._changeEventSource,
      options
    );
    await deltaCalculator.getDelta({
      reset: true,
      shallow: options.shallow,
    });
    const graph = deltaCalculator.getGraph();
    this._deltaCalculators.set(graph, deltaCalculator);
    return graph;
  }
  async getDelta(graph, { reset, shallow }) {
    const deltaCalculator = this._deltaCalculators.get(graph);
    if (!deltaCalculator) {
      throw new Error("Graph not found");
    }
    return await deltaCalculator.getDelta({
      reset,
      shallow,
    });
  }
  listen(graph, callback) {
    const deltaCalculator = this._deltaCalculators.get(graph);
    if (!deltaCalculator) {
      throw new Error("Graph not found");
    }
    deltaCalculator.on("change", callback);
    return () => {
      deltaCalculator.removeListener("change", callback);
    };
  }
  endGraph(graph) {
    const deltaCalculator = this._deltaCalculators.get(graph);
    if (!deltaCalculator) {
      throw new Error("Graph not found");
    }
    deltaCalculator.end();
    this._deltaCalculators.delete(graph);
  }
}
module.exports = DeltaBundler;
