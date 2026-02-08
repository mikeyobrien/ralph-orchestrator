"use strict";

var _Graph = require("./Graph");
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const debug = require("debug")("Metro:DeltaCalculator");
const { EventEmitter } = require("events");
class DeltaCalculator extends EventEmitter {
  _deletedFiles = new Set();
  _modifiedFiles = new Set();
  _addedFiles = new Set();
  _requiresReset = false;
  constructor(entryPoints, changeEventSource, options) {
    super();
    this._options = options;
    this._changeEventSource = changeEventSource;
    this._graph = new _Graph.Graph({
      entryPoints,
      transformOptions: this._options.transformOptions,
    });
    this._changeEventSource.on("change", this._handleMultipleFileChanges);
  }
  end() {
    this._changeEventSource.removeListener(
      "change",
      this._handleMultipleFileChanges
    );
    this.removeAllListeners();
    this._graph = new _Graph.Graph({
      entryPoints: this._graph.entryPoints,
      transformOptions: this._options.transformOptions,
    });
    this._modifiedFiles = new Set();
    this._deletedFiles = new Set();
    this._addedFiles = new Set();
  }
  async getDelta({ reset, shallow }) {
    debug("Calculating delta (reset: %s, shallow: %s)", reset, shallow);
    if (this._currentBuildPromise) {
      await this._currentBuildPromise;
    }
    const modifiedFiles = this._modifiedFiles;
    this._modifiedFiles = new Set();
    const deletedFiles = this._deletedFiles;
    this._deletedFiles = new Set();
    const addedFiles = this._addedFiles;
    this._addedFiles = new Set();
    const requiresReset = this._requiresReset;
    this._requiresReset = false;
    if (requiresReset) {
      const markModified = (file) => {
        if (!addedFiles.has(file) && !deletedFiles.has(file)) {
          modifiedFiles.add(file);
        }
      };
      this._graph.dependencies.forEach((_, key) => markModified(key));
      this._graph.entryPoints.forEach(markModified);
    }
    this._currentBuildPromise = this._getChangedDependencies(
      modifiedFiles,
      deletedFiles,
      addedFiles
    );
    let result;
    try {
      result = await this._currentBuildPromise;
    } catch (error) {
      modifiedFiles.forEach((file) => this._modifiedFiles.add(file));
      deletedFiles.forEach((file) => this._deletedFiles.add(file));
      addedFiles.forEach((file) => this._addedFiles.add(file));
      throw error;
    } finally {
      this._currentBuildPromise = null;
    }
    if (reset) {
      this._graph.reorderGraph({
        shallow,
      });
      return {
        added: this._graph.dependencies,
        modified: new Map(),
        deleted: new Set(),
        reset: true,
      };
    }
    return result;
  }
  getGraph() {
    return this._graph;
  }
  _handleMultipleFileChanges = (changeEvent) => {
    changeEvent.eventsQueue.forEach((eventInfo) => {
      this._handleFileChange(eventInfo, changeEvent.logger);
    });
  };
  _handleFileChange = ({ type, filePath, metadata }, logger) => {
    debug("Handling %s: %s (type: %s)", type, filePath, metadata.type);
    if (
      metadata.type === "l" ||
      (this._options.unstable_enablePackageExports &&
        filePath.endsWith(_path.default.sep + "package.json"))
    ) {
      this._requiresReset = true;
      this.emit("change", {
        logger,
      });
    }
    let state;
    if (this._deletedFiles.has(filePath)) {
      state = "deleted";
    } else if (this._modifiedFiles.has(filePath)) {
      state = "modified";
    } else if (this._addedFiles.has(filePath)) {
      state = "added";
    }
    let nextState;
    if (type === "delete") {
      nextState = "deleted";
    } else if (type === "add") {
      nextState = state === "deleted" ? "modified" : "added";
    } else {
      nextState = state === "added" ? "added" : "modified";
    }
    switch (nextState) {
      case "deleted":
        this._deletedFiles.add(filePath);
        this._modifiedFiles.delete(filePath);
        this._addedFiles.delete(filePath);
        break;
      case "added":
        this._addedFiles.add(filePath);
        this._deletedFiles.delete(filePath);
        this._modifiedFiles.delete(filePath);
        break;
      case "modified":
        this._modifiedFiles.add(filePath);
        this._deletedFiles.delete(filePath);
        this._addedFiles.delete(filePath);
        break;
      default:
        nextState;
    }
    this.emit("change", {
      logger,
    });
  };
  async _getChangedDependencies(modifiedFiles, deletedFiles, addedFiles) {
    if (!this._graph.dependencies.size) {
      const { added } = await this._graph.initialTraverseDependencies(
        this._options
      );
      return {
        added,
        modified: new Map(),
        deleted: new Set(),
        reset: true,
      };
    }
    deletedFiles.forEach((filePath) => {
      for (const modifiedModulePath of this._graph.getModifiedModulesForDeletedPath(
        filePath
      )) {
        if (!deletedFiles.has(modifiedModulePath)) {
          modifiedFiles.add(modifiedModulePath);
        }
      }
    });
    if (this._options.unstable_allowRequireContext) {
      addedFiles.forEach((filePath) => {
        this._graph.markModifiedContextModules(filePath, modifiedFiles);
      });
    }
    const modifiedDependencies = Array.from(modifiedFiles).filter((filePath) =>
      this._graph.dependencies.has(filePath)
    );
    if (modifiedDependencies.length === 0) {
      return {
        added: new Map(),
        modified: new Map(),
        deleted: new Set(),
        reset: false,
      };
    }
    debug("Traversing dependencies for %s paths", modifiedDependencies.length);
    const { added, modified, deleted } = await this._graph.traverseDependencies(
      modifiedDependencies,
      this._options
    );
    debug(
      "Calculated graph delta {added: %s, modified: %d, deleted: %d}",
      added.size,
      modified.size,
      deleted.size
    );
    return {
      added,
      modified,
      deleted,
      reset: false,
    };
  }
}
module.exports = DeltaCalculator;
