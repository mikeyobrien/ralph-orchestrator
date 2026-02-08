"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _Transformer = _interopRequireDefault(
  require("./DeltaBundler/Transformer"),
);
var _DependencyGraph = _interopRequireDefault(
  require("./node-haste/DependencyGraph"),
);
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class Bundler {
  constructor(config, options) {
    this._depGraph = new _DependencyGraph.default(config, options);
    this._initializedPromise = this._depGraph
      .ready()
      .then(() => {
        config.reporter.update({
          type: "transformer_load_started",
        });
        this._transformer = new _Transformer.default(config, {
          getOrComputeSha1: (filePath) =>
            this._depGraph.getOrComputeSha1(filePath),
        });
        config.reporter.update({
          type: "transformer_load_done",
        });
      })
      .catch((error) => {
        console.error("Failed to construct transformer: ", error);
        config.reporter.update({
          type: "transformer_load_failed",
          error,
        });
      });
  }
  getWatcher() {
    return this._depGraph.getWatcher();
  }
  async end() {
    await this.ready();
    await this._transformer.end();
    await this._depGraph.end();
  }
  async getDependencyGraph() {
    await this.ready();
    return this._depGraph;
  }
  async transformFile(filePath, transformOptions, fileBuffer) {
    await this.ready();
    return this._transformer.transformFile(
      filePath,
      transformOptions,
      fileBuffer,
    );
  }
  async ready() {
    await this._initializedPromise;
  }
}
exports.default = Bundler;
