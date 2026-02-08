"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _AbstractConsumer = _interopRequireDefault(require("./AbstractConsumer"));
var _constants = require("./constants");
var _normalizeSourcePath = _interopRequireDefault(
  require("./normalizeSourcePath"),
);
var _search = require("./search");
var _invariant = _interopRequireDefault(require("invariant"));
var _ob = require("ob1");
var _vlq = require("vlq");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class MappingsConsumer extends _AbstractConsumer.default {
  constructor(sourceMap) {
    super(sourceMap);
    this._sourceMap = sourceMap;
    this._decodedMappings = null;
    this._normalizedSources = null;
  }
  originalPositionFor(generatedPosition) {
    const { line, column } = generatedPosition;
    if (line == null || column == null) {
      return {
        ..._constants.EMPTY_POSITION,
      };
    }
    if (generatedPosition.bias != null) {
      (0, _invariant.default)(
        generatedPosition.bias === _constants.GREATEST_LOWER_BOUND,
        `Unimplemented lookup bias: ${(0, _constants.lookupBiasToString)(generatedPosition.bias)}`,
      );
    }
    const mappings = this._decodeAndCacheMappings();
    const index = (0, _search.greatestLowerBound)(
      mappings,
      {
        line,
        column,
      },
      (position, mapping) => {
        if (position.line === mapping.generatedLine) {
          return (0, _ob.get0)(
            (0, _ob.sub)(position.column, mapping.generatedColumn),
          );
        }
        return (0, _ob.get0)(
          (0, _ob.sub)(position.line, mapping.generatedLine),
        );
      },
    );
    if (
      index != null &&
      mappings[index].generatedLine === generatedPosition.line
    ) {
      const mapping = mappings[index];
      return {
        source: mapping.source,
        name: mapping.name,
        line: mapping.originalLine,
        column: mapping.originalColumn,
      };
    }
    return {
      ..._constants.EMPTY_POSITION,
    };
  }
  *_decodeMappings() {
    let generatedLine = _constants.FIRST_LINE;
    let generatedColumn = _constants.FIRST_COLUMN;
    let originalLine = _constants.FIRST_LINE;
    let originalColumn = _constants.FIRST_COLUMN;
    let nameIndex = (0, _ob.add0)(0);
    let sourceIndex = (0, _ob.add0)(0);
    const normalizedSources = this._normalizeAndCacheSources();
    const { mappings: mappingsRaw, names } = this._sourceMap;
    let next;
    const vlqCache = new Map();
    for (let i = 0; i < mappingsRaw.length; i = next) {
      switch (mappingsRaw[i]) {
        case ";":
          generatedLine = (0, _ob.inc)(generatedLine);
          generatedColumn = _constants.FIRST_COLUMN;
        case ",":
          next = i + 1;
          continue;
      }
      findNext: for (next = i + 1; next < mappingsRaw.length; ++next) {
        switch (mappingsRaw[next]) {
          case ";":
          case ",":
            break findNext;
        }
      }
      const mappingRaw = mappingsRaw.slice(i, next);
      let decodedVlqValues;
      if (vlqCache.has(mappingRaw)) {
        decodedVlqValues = vlqCache.get(mappingRaw);
      } else {
        decodedVlqValues = (0, _vlq.decode)(mappingRaw);
        vlqCache.set(mappingRaw, decodedVlqValues);
      }
      (0, _invariant.default)(
        Array.isArray(decodedVlqValues),
        "Decoding VLQ tuple failed",
      );
      const [
        generatedColumnDelta,
        sourceIndexDelta,
        originalLineDelta,
        originalColumnDelta,
        nameIndexDelta,
      ] = decodedVlqValues;
      (0, _vlq.decode)(mappingRaw);
      (0, _invariant.default)(
        generatedColumnDelta != null,
        "Invalid generated column delta",
      );
      generatedColumn = (0, _ob.add)(generatedColumn, generatedColumnDelta);
      const mapping = {
        generatedLine,
        generatedColumn,
        source: null,
        name: null,
        originalLine: null,
        originalColumn: null,
      };
      if (sourceIndexDelta != null) {
        sourceIndex = (0, _ob.add)(sourceIndex, sourceIndexDelta);
        mapping.source = normalizedSources[(0, _ob.get0)(sourceIndex)];
        (0, _invariant.default)(
          originalLineDelta != null,
          "Invalid original line delta",
        );
        (0, _invariant.default)(
          originalColumnDelta != null,
          "Invalid original column delta",
        );
        originalLine = (0, _ob.add)(originalLine, originalLineDelta);
        originalColumn = (0, _ob.add)(originalColumn, originalColumnDelta);
        mapping.originalLine = originalLine;
        mapping.originalColumn = originalColumn;
        if (nameIndexDelta != null) {
          nameIndex = (0, _ob.add)(nameIndex, nameIndexDelta);
          mapping.name = names[(0, _ob.get0)(nameIndex)];
        }
      }
      yield mapping;
    }
  }
  _normalizeAndCacheSources() {
    if (!this._normalizedSources) {
      this._normalizedSources = this._sourceMap.sources.map((source) =>
        (0, _normalizeSourcePath.default)(source, this._sourceMap),
      );
    }
    return this._normalizedSources;
  }
  _decodeAndCacheMappings() {
    if (!this._decodedMappings) {
      this._decodedMappings = [...this._decodeMappings()];
    }
    return this._decodedMappings;
  }
  generatedMappings() {
    return this._decodeAndCacheMappings();
  }
  _indexOfSource(source) {
    const idx = this._normalizeAndCacheSources().indexOf(
      (0, _normalizeSourcePath.default)(source, this._sourceMap),
    );
    if (idx === -1) {
      return null;
    }
    return (0, _ob.add0)(idx);
  }
  sourceContentFor(source, nullOnMissing) {
    const { sourcesContent } = this._sourceMap;
    if (!sourcesContent) {
      return null;
    }
    const idx = this._indexOfSource(source);
    if (idx == null) {
      return null;
    }
    return sourcesContent[(0, _ob.get0)(idx)] ?? null;
  }
}
exports.default = MappingsConsumer;
