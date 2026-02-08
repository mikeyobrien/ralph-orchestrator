"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
Object.defineProperty(exports, "BundleBuilder", {
  enumerable: true,
  get: function () {
    return _BundleBuilder.BundleBuilder;
  },
});
Object.defineProperty(exports, "Consumer", {
  enumerable: true,
  get: function () {
    return _Consumer.default;
  },
});
Object.defineProperty(exports, "composeSourceMaps", {
  enumerable: true,
  get: function () {
    return _composeSourceMaps.default;
  },
});
Object.defineProperty(exports, "createIndexMap", {
  enumerable: true,
  get: function () {
    return _BundleBuilder.createIndexMap;
  },
});
exports.default = void 0;
exports.fromRawMappings = fromRawMappings;
exports.fromRawMappingsNonBlocking = fromRawMappingsNonBlocking;
Object.defineProperty(exports, "functionMapBabelPlugin", {
  enumerable: true,
  get: function () {
    return _generateFunctionMap.functionMapBabelPlugin;
  },
});
Object.defineProperty(exports, "generateFunctionMap", {
  enumerable: true,
  get: function () {
    return _generateFunctionMap.generateFunctionMap;
  },
});
Object.defineProperty(exports, "normalizeSourcePath", {
  enumerable: true,
  get: function () {
    return _normalizeSourcePath.default;
  },
});
exports.toBabelSegments = toBabelSegments;
exports.toSegmentTuple = toSegmentTuple;
var _BundleBuilder = require("./BundleBuilder");
var _composeSourceMaps = _interopRequireDefault(require("./composeSourceMaps"));
var _Consumer = _interopRequireDefault(require("./Consumer"));
var _normalizeSourcePath = _interopRequireDefault(
  require("./Consumer/normalizeSourcePath"),
);
var _generateFunctionMap = require("./generateFunctionMap");
var _Generator = _interopRequireDefault(require("./Generator"));
var _sourceMap = _interopRequireDefault(require("source-map"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function fromRawMappingsImpl(isBlocking, onDone, modules, offsetLines) {
  const modulesToProcess = modules.slice();
  const generator = new _Generator.default();
  let carryOver = offsetLines;
  function processNextModule() {
    if (modulesToProcess.length === 0) {
      return true;
    }
    const mod = modulesToProcess.shift();
    const { code, map } = mod;
    if (Array.isArray(map)) {
      addMappingsForFile(generator, map, mod, carryOver);
    } else if (map != null) {
      throw new Error(
        `Unexpected module with full source map found: ${mod.path}`,
      );
    }
    carryOver = carryOver + countLines(code);
    return false;
  }
  function workLoop() {
    const time = process.hrtime();
    while (true) {
      const isDone = processNextModule();
      if (isDone) {
        onDone(generator);
        break;
      }
      if (!isBlocking) {
        const diff = process.hrtime(time);
        const NS_IN_MS = 1000000;
        if (diff[1] > 50 * NS_IN_MS) {
          setImmediate(workLoop);
          break;
        }
      }
    }
  }
  workLoop();
}
function fromRawMappings(modules, offsetLines = 0) {
  let generator;
  fromRawMappingsImpl(
    true,
    (g) => {
      generator = g;
    },
    modules,
    offsetLines,
  );
  if (generator == null) {
    throw new Error("Expected fromRawMappingsImpl() to finish synchronously.");
  }
  return generator;
}
async function fromRawMappingsNonBlocking(modules, offsetLines = 0) {
  return new Promise((resolve) => {
    fromRawMappingsImpl(false, resolve, modules, offsetLines);
  });
}
function toBabelSegments(sourceMap) {
  const rawMappings = [];
  new _sourceMap.default.SourceMapConsumer(sourceMap).eachMapping((map) => {
    rawMappings.push(
      map.originalLine == null || map.originalColumn == null
        ? {
            generated: {
              line: map.generatedLine,
              column: map.generatedColumn,
            },
            source: map.source,
            name: map.name,
          }
        : {
            generated: {
              line: map.generatedLine,
              column: map.generatedColumn,
            },
            original: {
              line: map.originalLine,
              column: map.originalColumn,
            },
            source: map.source,
            name: map.name,
          },
    );
  });
  return rawMappings;
}
function toSegmentTuple(mapping) {
  const { column, line } = mapping.generated;
  const { name, original } = mapping;
  if (original == null) {
    return [line, column];
  }
  if (typeof name !== "string") {
    return [line, column, original.line, original.column];
  }
  return [line, column, original.line, original.column, name];
}
function addMappingsForFile(generator, mappings, module, carryOver) {
  generator.startFile(module.path, module.source, module.functionMap, {
    addToIgnoreList: module.isIgnored,
  });
  for (let i = 0, n = mappings.length; i < n; ++i) {
    addMapping(generator, mappings[i], carryOver);
  }
  generator.endFile();
}
function addMapping(generator, mapping, carryOver) {
  const line = mapping[0] + carryOver;
  const column = mapping[1];
  switch (mapping.length) {
    case 2:
      generator.addSimpleMapping(line, column);
      return;
    case 4:
      generator.addSourceMapping(line, column, mapping[2], mapping[3]);
      return;
    case 5:
      generator.addNamedSourceMapping(
        line,
        column,
        mapping[2],
        mapping[3],
        mapping[4],
      );
      return;
  }
  throw new Error(`Invalid mapping: [${mapping.join(", ")}]`);
}
const newline = /\r\n?|\n|\u2028|\u2029/g;
const countLines = (string) => (string.match(newline) || []).length + 1;
var _default = (exports.default = {
  BundleBuilder: _BundleBuilder.BundleBuilder,
  composeSourceMaps: _composeSourceMaps.default,
  Consumer: _Consumer.default,
  createIndexMap: _BundleBuilder.createIndexMap,
  generateFunctionMap: _generateFunctionMap.generateFunctionMap,
  fromRawMappings,
  fromRawMappingsNonBlocking,
  functionMapBabelPlugin: _generateFunctionMap.functionMapBabelPlugin,
  normalizeSourcePath: _normalizeSourcePath.default,
  toBabelSegments,
  toSegmentTuple,
});
