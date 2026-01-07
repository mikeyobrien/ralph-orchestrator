"use strict";

const B64Builder = require("./B64Builder");
class Generator {
  constructor() {
    this.builder = new B64Builder();
    this.last = {
      generatedColumn: 0,
      generatedLine: 1,
      name: 0,
      source: 0,
      sourceColumn: 0,
      sourceLine: 1,
    };
    this.names = new IndexedSet();
    this.source = -1;
    this.sources = [];
    this.sourcesContent = [];
    this.x_facebook_sources = [];
    this.x_google_ignoreList = [];
  }
  startFile(file, code, functionMap, flags) {
    const { addToIgnoreList = false } = flags ?? {};
    const sourceIndex = this.sources.push(file) - 1;
    this.source = sourceIndex;
    this.sourcesContent.push(code);
    this.x_facebook_sources.push(functionMap ? [functionMap] : null);
    if (addToIgnoreList) {
      this.x_google_ignoreList.push(sourceIndex);
    }
  }
  endFile() {
    this.source = -1;
  }
  addSimpleMapping(generatedLine, generatedColumn) {
    const last = this.last;
    if (
      this.source === -1 ||
      (generatedLine === last.generatedLine &&
        generatedColumn < last.generatedColumn) ||
      generatedLine < last.generatedLine
    ) {
      const msg =
        this.source === -1
          ? "Cannot add mapping before starting a file with `addFile()`"
          : "Mapping is for a position preceding an earlier mapping";
      throw new Error(msg);
    }
    if (generatedLine > last.generatedLine) {
      this.builder.markLines(generatedLine - last.generatedLine);
      last.generatedLine = generatedLine;
      last.generatedColumn = 0;
    }
    this.builder.startSegment(generatedColumn - last.generatedColumn);
    last.generatedColumn = generatedColumn;
  }
  addSourceMapping(generatedLine, generatedColumn, sourceLine, sourceColumn) {
    this.addSimpleMapping(generatedLine, generatedColumn);
    const last = this.last;
    this.builder
      .append(this.source - last.source)
      .append(sourceLine - last.sourceLine)
      .append(sourceColumn - last.sourceColumn);
    last.source = this.source;
    last.sourceColumn = sourceColumn;
    last.sourceLine = sourceLine;
  }
  addNamedSourceMapping(
    generatedLine,
    generatedColumn,
    sourceLine,
    sourceColumn,
    name
  ) {
    this.addSourceMapping(
      generatedLine,
      generatedColumn,
      sourceLine,
      sourceColumn
    );
    const last = this.last;
    const nameIndex = this.names.indexFor(name);
    this.builder.append(nameIndex - last.name);
    last.name = nameIndex;
  }
  toMap(file, options) {
    const content =
      options && options.excludeSource === true
        ? {}
        : {
            sourcesContent: this.sourcesContent.slice(),
          };
    const sourcesMetadata = this.hasSourcesMetadata()
      ? {
          x_facebook_sources: JSON.parse(
            JSON.stringify(this.x_facebook_sources)
          ),
        }
      : {};
    const ignoreList = this.x_google_ignoreList.length
      ? {
          x_google_ignoreList: this.x_google_ignoreList,
        }
      : {};
    return {
      version: 3,
      file,
      sources: this.sources.slice(),
      ...content,
      ...sourcesMetadata,
      ...ignoreList,
      names: this.names.items(),
      mappings: this.builder.toString(),
    };
  }
  toString(file, options) {
    let content;
    if (options && options.excludeSource === true) {
      content = "";
    } else {
      content = `"sourcesContent":${JSON.stringify(this.sourcesContent)},`;
    }
    let sourcesMetadata;
    if (this.hasSourcesMetadata()) {
      sourcesMetadata = `"x_facebook_sources":${JSON.stringify(
        this.x_facebook_sources
      )},`;
    } else {
      sourcesMetadata = "";
    }
    let ignoreList;
    if (this.x_google_ignoreList.length) {
      ignoreList = `"x_google_ignoreList":${JSON.stringify(
        this.x_google_ignoreList
      )},`;
    } else {
      ignoreList = "";
    }
    return (
      "{" +
      '"version":3,' +
      (file != null ? `"file":${JSON.stringify(file)},` : "") +
      `"sources":${JSON.stringify(this.sources)},` +
      content +
      sourcesMetadata +
      ignoreList +
      `"names":${JSON.stringify(this.names.items())},` +
      `"mappings":"${this.builder.toString()}"` +
      "}"
    );
  }
  hasSourcesMetadata() {
    return this.x_facebook_sources.some(
      (metadata) => metadata != null && metadata.some((value) => value != null)
    );
  }
}
class IndexedSet {
  constructor() {
    this.map = new Map();
    this.nextIndex = 0;
  }
  indexFor(x) {
    let index = this.map.get(x);
    if (index == null) {
      index = this.nextIndex++;
      this.map.set(x, index);
    }
    return index;
  }
  items() {
    return Array.from(this.map.keys());
  }
}
module.exports = Generator;
