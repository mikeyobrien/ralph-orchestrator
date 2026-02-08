"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = symbolicate;
var _search = require("metro-source-map/private/Consumer/search");
var _Symbolication = require("metro-symbolicate/private/Symbolication");
function createFunctionNameGetter(module) {
  const consumer = new _Symbolication.SourceMetadataMapConsumer(
    {
      version: 3,
      mappings: "",
      sources: ["dummy"],
      names: [],
      x_facebook_sources: [[module.functionMap]],
    },
    (name) => name,
  );
  return ({ line1Based, column0Based }) =>
    consumer.functionNameFor({
      line: line1Based,
      column: column0Based,
      source: "dummy",
    });
}
async function symbolicate(stack, maps, config, extraData) {
  const mapsByUrl = new Map();
  for (const [url, map] of maps) {
    mapsByUrl.set(url, map);
  }
  const functionNameGetters = new Map();
  function findModule(frame) {
    const map = mapsByUrl.get(frame.file);
    if (!map || frame.lineNumber == null) {
      return null;
    }
    const moduleIndex = (0, _search.greatestLowerBound)(
      map,
      frame.lineNumber,
      (target, candidate) => target - candidate.firstLine1Based,
    );
    if (moduleIndex == null) {
      return null;
    }
    return map[moduleIndex];
  }
  function findOriginalPos(frame, module) {
    if (
      module.map == null ||
      frame.lineNumber == null ||
      frame.column == null
    ) {
      return null;
    }
    const generatedPosInModule = {
      line1Based: frame.lineNumber - module.firstLine1Based + 1,
      column0Based: frame.column,
    };
    const mappingIndex = (0, _search.greatestLowerBound)(
      module.map,
      generatedPosInModule,
      (target, candidate) => {
        if (target.line1Based === candidate[0]) {
          return target.column0Based - candidate[1];
        }
        return target.line1Based - candidate[0];
      },
    );
    if (mappingIndex == null) {
      return null;
    }
    const mapping = module.map[mappingIndex];
    if (mapping[0] !== generatedPosInModule.line1Based || mapping.length < 4) {
      return null;
    }
    return {
      line1Based: mapping[2],
      column0Based: mapping[3],
    };
  }
  function findFunctionName(originalPos, module) {
    if (module.functionMap) {
      let getFunctionName = functionNameGetters.get(module);
      if (!getFunctionName) {
        getFunctionName = createFunctionNameGetter(module);
        functionNameGetters.set(module, getFunctionName);
      }
      return getFunctionName(originalPos);
    }
    return null;
  }
  function symbolicateFrame(frame) {
    const module = findModule(frame);
    if (!module) {
      return {
        ...frame,
      };
    }
    if (!Array.isArray(module.map)) {
      throw new Error(
        `Unexpected module with serialized source map found: ${module.path}`,
      );
    }
    const originalPos = findOriginalPos(frame, module);
    if (!originalPos) {
      return {
        ...frame,
      };
    }
    const methodName =
      findFunctionName(originalPos, module) ?? frame.methodName;
    return {
      ...frame,
      methodName,
      file: module.path,
      lineNumber: originalPos.line1Based,
      column: originalPos.column0Based,
    };
  }
  async function customizeFrame(frame) {
    const customizations =
      (await config.symbolicator.customizeFrame(frame)) || {};
    return {
      ...frame,
      ...customizations,
    };
  }
  async function customizeStack(symbolicatedStack) {
    return await config.symbolicator.customizeStack(
      symbolicatedStack,
      extraData,
    );
  }
  return Promise.all(stack.map(symbolicateFrame).map(customizeFrame)).then(
    customizeStack,
  );
}
