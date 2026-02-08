"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = composeSourceMaps;
var _Consumer = _interopRequireDefault(require("./Consumer"));
var _sourceMap = require("source-map");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
_Consumer.default;
function composeSourceMaps(maps) {
  const SourceMetadataMapConsumer =
    require("metro-symbolicate/private/SourceMetadataMapConsumer").default;
  const GoogleIgnoreListConsumer =
    require("metro-symbolicate/private/GoogleIgnoreListConsumer").default;
  if (maps.length < 1) {
    throw new Error("composeSourceMaps: Expected at least one map");
  }
  const firstMap = maps[0];
  const consumers = maps
    .map(function (map) {
      return new _Consumer.default(map);
    })
    .reverse();
  const generator = new _sourceMap.SourceMapGenerator({
    file: consumers[0].file,
  });
  consumers[0].eachMapping((mapping) => {
    const original = findOriginalPosition(
      consumers,
      mapping.generatedLine,
      mapping.generatedColumn,
    );
    generator.addMapping({
      generated: {
        line: mapping.generatedLine,
        column: mapping.generatedColumn,
      },
      original:
        original.line != null
          ? {
              line: original.line,
              column: original.column,
            }
          : null,
      source: original.source,
      name: original.name,
    });
  });
  const composedMap = generator.toJSON();
  composedMap.sourcesContent = composedMap.sources.map((source) =>
    consumers[consumers.length - 1].sourceContentFor(source, true),
  );
  if (composedMap.sourcesContent.every((content) => content == null)) {
    delete composedMap.sourcesContent;
  }
  const metadataConsumer = new SourceMetadataMapConsumer(firstMap);
  composedMap.x_facebook_sources = metadataConsumer.toArray(
    composedMap.sources,
  );
  const function_offsets = maps[maps.length - 1].x_hermes_function_offsets;
  if (function_offsets) {
    composedMap.x_hermes_function_offsets = function_offsets;
  }
  const ignoreListConsumer = new GoogleIgnoreListConsumer(firstMap);
  const x_google_ignoreList = ignoreListConsumer.toArray(composedMap.sources);
  if (x_google_ignoreList.length) {
    composedMap.x_google_ignoreList = x_google_ignoreList;
  }
  return composedMap;
}
function findOriginalPosition(consumers, generatedLine, generatedColumn) {
  let currentLine = generatedLine;
  let currentColumn = generatedColumn;
  let original = {
    line: null,
    column: null,
    source: null,
    name: null,
  };
  for (const consumer of consumers) {
    if (currentLine == null || currentColumn == null) {
      return {
        line: null,
        column: null,
        source: null,
        name: null,
      };
    }
    original = consumer.originalPositionFor({
      line: currentLine,
      column: currentColumn,
    });
    currentLine = original.line;
    currentColumn = original.column;
    if (currentLine == null) {
      return {
        line: null,
        column: null,
        source: null,
        name: null,
      };
    }
  }
  return original;
}
