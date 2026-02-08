"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = exports.CACHE_VERSION = void 0;
var _normalizePathSeparatorsToPosix = _interopRequireDefault(
  require("../lib/normalizePathSeparatorsToPosix"),
);
var _normalizePathSeparatorsToSystem = _interopRequireDefault(
  require("../lib/normalizePathSeparatorsToSystem"),
);
var _RootPathUtils = require("../lib/RootPathUtils");
var _getMockName = _interopRequireDefault(require("./mocks/getMockName"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const CACHE_VERSION = (exports.CACHE_VERSION = 2);
class MockPlugin {
  name = "mocks";
  #mocksPattern;
  #raw;
  #rootDir;
  #pathUtils;
  #console;
  #throwOnModuleCollision;
  constructor({
    console,
    mocksPattern,
    rawMockMap = {
      mocks: new Map(),
      duplicates: new Map(),
      version: CACHE_VERSION,
    },
    rootDir,
    throwOnModuleCollision,
  }) {
    this.#mocksPattern = mocksPattern;
    if (rawMockMap.version !== CACHE_VERSION) {
      throw new Error("Incompatible state passed to MockPlugin");
    }
    this.#raw = rawMockMap;
    this.#rootDir = rootDir;
    this.#console = console;
    this.#pathUtils = new _RootPathUtils.RootPathUtils(rootDir);
    this.#throwOnModuleCollision = throwOnModuleCollision;
  }
  async initialize({ files, pluginState }) {
    if (pluginState != null && pluginState.version === this.#raw.version) {
      this.#raw = pluginState;
    } else {
      await this.bulkUpdate({
        addedOrModified: [
          ...files.metadataIterator({
            includeNodeModules: false,
            includeSymlinks: false,
          }),
        ].map(({ canonicalPath, metadata }) => [canonicalPath, metadata]),
        removed: [],
      });
    }
  }
  getMockModule(name) {
    const mockPosixRelativePath =
      this.#raw.mocks.get(name) || this.#raw.mocks.get(name + "/index");
    if (typeof mockPosixRelativePath !== "string") {
      return null;
    }
    return this.#pathUtils.normalToAbsolute(
      (0, _normalizePathSeparatorsToSystem.default)(mockPosixRelativePath),
    );
  }
  async bulkUpdate(delta) {
    for (const [relativeFilePath] of delta.removed) {
      this.onRemovedFile(relativeFilePath);
    }
    for (const [relativeFilePath] of delta.addedOrModified) {
      this.onNewOrModifiedFile(relativeFilePath);
    }
  }
  onNewOrModifiedFile(relativeFilePath) {
    const absoluteFilePath = this.#pathUtils.normalToAbsolute(relativeFilePath);
    if (!this.#mocksPattern.test(absoluteFilePath)) {
      return;
    }
    const mockName = (0, _getMockName.default)(absoluteFilePath);
    const posixRelativePath = (0, _normalizePathSeparatorsToPosix.default)(
      relativeFilePath,
    );
    const existingMockPosixPath = this.#raw.mocks.get(mockName);
    if (existingMockPosixPath != null) {
      if (existingMockPosixPath !== posixRelativePath) {
        let duplicates = this.#raw.duplicates.get(mockName);
        if (duplicates == null) {
          duplicates = new Set([existingMockPosixPath, posixRelativePath]);
          this.#raw.duplicates.set(mockName, duplicates);
        } else {
          duplicates.add(posixRelativePath);
        }
        this.#console.warn(this.#getMessageForDuplicates(mockName, duplicates));
      }
    }
    this.#raw.mocks.set(mockName, posixRelativePath);
  }
  onRemovedFile(relativeFilePath) {
    const absoluteFilePath = this.#pathUtils.normalToAbsolute(relativeFilePath);
    if (!this.#mocksPattern.test(absoluteFilePath)) {
      return;
    }
    const mockName = (0, _getMockName.default)(absoluteFilePath);
    const duplicates = this.#raw.duplicates.get(mockName);
    if (duplicates != null) {
      const posixRelativePath = (0, _normalizePathSeparatorsToPosix.default)(
        relativeFilePath,
      );
      duplicates.delete(posixRelativePath);
      if (duplicates.size === 1) {
        this.#raw.duplicates.delete(mockName);
      }
      const remaining = (0, _nullthrows.default)(
        duplicates.values().next().value,
      );
      this.#raw.mocks.set(mockName, remaining);
    } else {
      this.#raw.mocks.delete(mockName);
    }
  }
  getSerializableSnapshot() {
    return {
      mocks: new Map(this.#raw.mocks),
      duplicates: new Map(
        [...this.#raw.duplicates].map(([k, v]) => [k, new Set(v)]),
      ),
      version: this.#raw.version,
    };
  }
  assertValid() {
    if (!this.#throwOnModuleCollision) {
      return;
    }
    const errors = [];
    for (const [mockName, relativePosixPaths] of this.#raw.duplicates) {
      errors.push(this.#getMessageForDuplicates(mockName, relativePosixPaths));
    }
    if (errors.length > 0) {
      throw new Error(
        `Mock map has ${errors.length} error${errors.length > 1 ? "s" : ""}:\n${errors.join("\n")}`,
      );
    }
  }
  #getMessageForDuplicates(mockName, relativePosixPaths) {
    return (
      "Duplicate manual mock found for `" +
      mockName +
      "`:\n" +
      [...relativePosixPaths]
        .map(
          (relativePosixPath) =>
            "    * <rootDir>" +
            _path.default.sep +
            this.#pathUtils.absoluteToNormal(
              (0, _normalizePathSeparatorsToSystem.default)(relativePosixPath),
            ) +
            "\n",
        )
        .join("")
    );
  }
  getCacheKey() {
    return (
      this.#mocksPattern.source.replaceAll("\\\\", "\\/") +
      "," +
      this.#mocksPattern.flags
    );
  }
}
exports.default = MockPlugin;
