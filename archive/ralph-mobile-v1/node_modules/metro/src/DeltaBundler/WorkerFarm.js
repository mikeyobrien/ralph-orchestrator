"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _jestWorker = require("jest-worker");
var _metroCore = require("metro-core");
class WorkerFarm {
  constructor(config, transformerConfig) {
    this._config = config;
    this._transformerConfig = transformerConfig;
    const absoluteWorkerPath = require.resolve("./Worker.js");
    if (this._config.maxWorkers > 1) {
      const worker = this._makeFarm(
        absoluteWorkerPath,
        ["transform"],
        this._config.maxWorkers,
      );
      worker.getStdout().on("data", (chunk) => {
        this._config.reporter.update({
          type: "worker_stdout_chunk",
          chunk: chunk.toString("utf8"),
        });
      });
      worker.getStderr().on("data", (chunk) => {
        this._config.reporter.update({
          type: "worker_stderr_chunk",
          chunk: chunk.toString("utf8"),
        });
      });
      this._worker = worker;
    } else {
      this._worker = require("./Worker");
    }
  }
  async kill() {
    if (this._worker && typeof this._worker.end === "function") {
      await this._worker.end();
    }
  }
  async transform(filename, options, fileBuffer) {
    try {
      const data = await this._worker.transform(
        filename,
        options,
        this._config.projectRoot,
        this._transformerConfig,
        fileBuffer,
      );
      _metroCore.Logger.log(data.transformFileStartLogEntry);
      _metroCore.Logger.log(data.transformFileEndLogEntry);
      return {
        result: data.result,
        sha1: data.sha1,
      };
    } catch (err) {
      if (err.loc) {
        throw this._formatBabelError(err, filename);
      } else {
        throw this._formatGenericError(err, filename);
      }
    }
  }
  _makeFarm(absoluteWorkerPath, exposedMethods, numWorkers) {
    const env = {
      ...process.env,
      FORCE_COLOR: 1,
    };
    return new _jestWorker.Worker(absoluteWorkerPath, {
      computeWorkerKey: this._config.stickyWorkers
        ? this._computeWorkerKey
        : undefined,
      exposedMethods,
      enableWorkerThreads: this._config.transformer.unstable_workerThreads,
      forkOptions: {
        env,
      },
      numWorkers,
      workerSchedulingPolicy: "in-order",
    });
  }
  _computeWorkerKey(method, filename) {
    if (method === "transform") {
      return filename;
    }
    return null;
  }
  _formatGenericError(err, filename) {
    const error = new TransformError(`${filename}: ${err.message}`);
    return Object.assign(error, {
      stack: (err.stack || "").split("\n").slice(0, -1).join("\n"),
      lineNumber: 0,
    });
  }
  _formatBabelError(err, filename) {
    const error = new TransformError(
      `${err.type || "Error"}${err.message.includes(filename) ? "" : " in " + filename}: ${err.message}`,
    );
    return Object.assign(error, {
      stack: err.stack,
      snippet: err.codeFrame,
      lineNumber: err.loc.line,
      column: err.loc.column,
      filename,
    });
  }
}
exports.default = WorkerFarm;
class TransformError extends SyntaxError {
  type = "TransformError";
  constructor(message) {
    super(message);
    Error.captureStackTrace && Error.captureStackTrace(this, TransformError);
  }
}
