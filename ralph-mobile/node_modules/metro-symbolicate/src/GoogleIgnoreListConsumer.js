"use strict";

const { normalizeSourcePath } = require("metro-source-map");
class GoogleIgnoreListConsumer {
  constructor(map, normalizeSourceFn = normalizeSourcePath) {
    this._sourceMap = map;
    this._normalizeSource = normalizeSourceFn;
  }
  isIgnored({ source }) {
    return source != null && this._getIgnoredSourceSet().has(source);
  }
  toArray(sources) {
    const ignoredSourceSet = this._getIgnoredSourceSet();
    const encoded = [];
    for (const [sourceIndex, source] of sources.entries()) {
      if (source != null && ignoredSourceSet.has(source)) {
        encoded.push(sourceIndex);
      }
    }
    return encoded;
  }
  _getIgnoredSourceSet() {
    if (!this._ignoredSourceSet) {
      const ignoredSourceSet = new Set();
      this._buildIgnoredSourceSet(this._sourceMap, ignoredSourceSet);
      this._ignoredSourceSet = ignoredSourceSet;
    }
    return this._ignoredSourceSet;
  }
  _buildIgnoredSourceSet(map, ignoredSourceSet) {
    if (map.mappings === undefined) {
      const indexMap = map;
      indexMap.sections.forEach((section) =>
        this._buildIgnoredSourceSet(section.map, ignoredSourceSet)
      );
      return;
    }
    if ("x_google_ignoreList" in map) {
      const basicMap = map;
      (basicMap.x_google_ignoreList || []).forEach((sourceIndex) => {
        let source = basicMap.sources[sourceIndex];
        if (source != null) {
          source = this._normalizeSource(source, basicMap);
          ignoredSourceSet.add(source);
        }
      });
    }
  }
}
module.exports = GoogleIgnoreListConsumer;
