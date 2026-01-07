"use strict";

const AbstractConsumer = require("./AbstractConsumer");
const {
  EMPTY_POSITION,
  FIRST_COLUMN,
  FIRST_LINE,
  GREATEST_LOWER_BOUND,
  lookupBiasToString,
} = require("./constants");
const normalizeSourcePath = require("./normalizeSourcePath");
const { greatestLowerBound } = require("./search");
const invariant = require("invariant");
const { add, add0, get0, inc, sub } = require("ob1");
const { decode: decodeVlq } = require("vlq");
class MappingsConsumer extends AbstractConsumer {
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
        ...EMPTY_POSITION,
      };
    }
    if (generatedPosition.bias != null) {
      invariant(
        generatedPosition.bias === GREATEST_LOWER_BOUND,
        `Unimplemented lookup bias: ${lookupBiasToString(
          generatedPosition.bias
        )}`
      );
    }
    const mappings = this._decodeAndCacheMappings();
    const index = greatestLowerBound(
      mappings,
      {
        line,
        column,
      },
      (position, mapping) => {
        if (position.line === mapping.generatedLine) {
          return get0(sub(position.column, mapping.generatedColumn));
        }
        return get0(sub(position.line, mapping.generatedLine));
      }
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
      ...EMPTY_POSITION,
    };
  }
  *_decodeMappings() {
    let generatedLine = FIRST_LINE;
    let generatedColumn = FIRST_COLUMN;
    let originalLine = FIRST_LINE;
    let originalColumn = FIRST_COLUMN;
    let nameIndex = add0(0);
    let sourceIndex = add0(0);
    const normalizedSources = this._normalizeAndCacheSources();
    const { mappings: mappingsRaw, names } = this._sourceMap;
    let next;
    const vlqCache = new Map();
    for (let i = 0; i < mappingsRaw.length; i = next) {
      switch (mappingsRaw[i]) {
        case ";":
          generatedLine = inc(generatedLine);
          generatedColumn = FIRST_COLUMN;
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
        decodedVlqValues = decodeVlq(mappingRaw);
        vlqCache.set(mappingRaw, decodedVlqValues);
      }
      invariant(Array.isArray(decodedVlqValues), "Decoding VLQ tuple failed");
      const [
        generatedColumnDelta,
        sourceIndexDelta,
        originalLineDelta,
        originalColumnDelta,
        nameIndexDelta,
      ] = decodedVlqValues;
      decodeVlq(mappingRaw);
      invariant(generatedColumnDelta != null, "Invalid generated column delta");
      generatedColumn = add(generatedColumn, generatedColumnDelta);
      const mapping = {
        generatedLine,
        generatedColumn,
        source: null,
        name: null,
        originalLine: null,
        originalColumn: null,
      };
      if (sourceIndexDelta != null) {
        sourceIndex = add(sourceIndex, sourceIndexDelta);
        mapping.source = normalizedSources[get0(sourceIndex)];
        invariant(originalLineDelta != null, "Invalid original line delta");
        invariant(originalColumnDelta != null, "Invalid original column delta");
        originalLine = add(originalLine, originalLineDelta);
        originalColumn = add(originalColumn, originalColumnDelta);
        mapping.originalLine = originalLine;
        mapping.originalColumn = originalColumn;
        if (nameIndexDelta != null) {
          nameIndex = add(nameIndex, nameIndexDelta);
          mapping.name = names[get0(nameIndex)];
        }
      }
      yield mapping;
    }
  }
  _normalizeAndCacheSources() {
    if (!this._normalizedSources) {
      this._normalizedSources = this._sourceMap.sources.map((source) =>
        normalizeSourcePath(source, this._sourceMap)
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
      normalizeSourcePath(source, this._sourceMap)
    );
    if (idx === -1) {
      return null;
    }
    return add0(idx);
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
    return sourcesContent[get0(idx)] ?? null;
  }
}
module.exports = MappingsConsumer;
