"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _metroSourceMap = require("metro-source-map");
var vlq = _interopRequireWildcard(require("vlq"));
function _getRequireWildcardCache(e) {
  if ("function" != typeof WeakMap) return null;
  var r = new WeakMap(),
    t = new WeakMap();
  return (_getRequireWildcardCache = function (e) {
    return e ? t : r;
  })(e);
}
function _interopRequireWildcard(e, r) {
  if (!r && e && e.__esModule) return e;
  if (null === e || ("object" != typeof e && "function" != typeof e))
    return { default: e };
  var t = _getRequireWildcardCache(r);
  if (t && t.has(e)) return t.get(e);
  var n = { __proto__: null },
    a = Object.defineProperty && Object.getOwnPropertyDescriptor;
  for (var u in e)
    if ("default" !== u && {}.hasOwnProperty.call(e, u)) {
      var i = a ? Object.getOwnPropertyDescriptor(e, u) : null;
      i && (i.get || i.set) ? Object.defineProperty(n, u, i) : (n[u] = e[u]);
    }
  return ((n.default = e), t && t.set(e, n), n);
}
const METADATA_FIELD_FUNCTIONS = 0;
class SourceMetadataMapConsumer {
  constructor(map, normalizeSourceFn = _metroSourceMap.normalizeSourcePath) {
    this._sourceMap = map;
    this._decodedFunctionMapCache = new Map();
    this._normalizeSource = normalizeSourceFn;
  }
  functionNameFor({ line, column, source }) {
    if (source && line != null && column != null) {
      const mappings = this._getFunctionMappings(source);
      if (mappings) {
        const mapping = findEnclosingMapping(mappings, {
          line,
          column,
        });
        if (mapping) {
          return mapping.name;
        }
      }
    }
    return null;
  }
  toArray(sources) {
    const metadataBySource = this._getMetadataBySource();
    const encoded = [];
    for (const source of sources) {
      encoded.push(metadataBySource[source] || null);
    }
    return encoded;
  }
  _getMetadataBySource() {
    if (!this._metadataBySource) {
      this._metadataBySource = this._getMetadataObjectsBySourceNames(
        this._sourceMap,
      );
    }
    return this._metadataBySource;
  }
  _getFunctionMappings(source) {
    if (this._decodedFunctionMapCache.has(source)) {
      return this._decodedFunctionMapCache.get(source);
    }
    let parsedFunctionMap = null;
    const metadataBySource = this._getMetadataBySource();
    if (Object.prototype.hasOwnProperty.call(metadataBySource, source)) {
      const metadata = metadataBySource[source] || [];
      parsedFunctionMap = decodeFunctionMap(metadata[METADATA_FIELD_FUNCTIONS]);
    }
    this._decodedFunctionMapCache.set(source, parsedFunctionMap);
    return parsedFunctionMap;
  }
  _getMetadataObjectsBySourceNames(map) {
    if (map.mappings === undefined) {
      const indexMap = map;
      return Object.assign(
        {},
        ...indexMap.sections.map((section) =>
          this._getMetadataObjectsBySourceNames(section.map),
        ),
      );
    }
    if ("x_facebook_sources" in map) {
      const basicMap = map;
      return (basicMap.x_facebook_sources || []).reduce(
        (acc, metadata, index) => {
          let source = basicMap.sources[index];
          if (source != null) {
            source = this._normalizeSource(source, basicMap);
            acc[source] = metadata;
          }
          return acc;
        },
        {},
      );
    }
    return {};
  }
}
exports.default = SourceMetadataMapConsumer;
function decodeFunctionMap(functionMap) {
  if (!functionMap) {
    return [];
  }
  const parsed = [];
  let line = 1;
  let nameIndex = 0;
  for (const lineMappings of functionMap.mappings.split(";")) {
    let column = 0;
    for (const mapping of lineMappings.split(",")) {
      const [columnDelta, nameDelta, lineDelta = 0] = vlq.decode(mapping);
      line += lineDelta;
      nameIndex += nameDelta;
      column += columnDelta;
      parsed.push({
        line,
        column,
        name: functionMap.names[nameIndex],
      });
    }
  }
  return parsed;
}
function findEnclosingMapping(mappings, target) {
  let first = 0;
  let it = 0;
  let count = mappings.length;
  let step;
  while (count > 0) {
    it = first;
    step = Math.floor(count / 2);
    it += step;
    if (comparePositions(target, mappings[it]) >= 0) {
      first = ++it;
      count -= step + 1;
    } else {
      count = step;
    }
  }
  return first ? mappings[first - 1] : null;
}
function comparePositions(a, b) {
  if (a.line === b.line) {
    return a.column - b.column;
  }
  return a.line - b.line;
}
