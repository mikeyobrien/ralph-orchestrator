"use strict";

const { ChromeHeapSnapshotProcessor } = require("./ChromeHeapSnapshot");
const GoogleIgnoreListConsumer = require("./GoogleIgnoreListConsumer");
const SourceMetadataMapConsumer = require("./SourceMetadataMapConsumer");
const fs = require("fs");
const invariant = require("invariant");
const nullthrows = require("nullthrows");
const path = require("path");
const UNKNOWN_MODULE_IDS = {
  segmentId: 0,
  localId: undefined,
};
class SymbolicationContext {
  constructor(options) {
    this.options = {
      inputLineStart: 1,
      inputColumnStart: 0,
      outputLineStart: 1,
      outputColumnStart: 0,
      nameSource: "function_names",
    };
    if (options) {
      for (const option of [
        "inputLineStart",
        "inputColumnStart",
        "outputLineStart",
        "outputColumnStart",
      ]) {
        if (options[option] != null) {
          this.options[option] = options[option];
        }
      }
      if (options.nameSource != null) {
        this.options.nameSource = options.nameSource;
      }
    }
  }
  symbolicate(stackTrace) {
    return stackTrace.replace(
      /(?:([^@: \n(]+)(@|:))?(?:(?:([^@: \n(]+):)?(\d+):(\d+)|\[native code\])/g,
      (match, func, delimiter, fileName, line, column) => {
        if (delimiter === ":" && func && !fileName) {
          fileName = func;
          func = null;
        }
        const original = this.getOriginalPositionFor(
          line,
          column,
          this.parseFileName(fileName || "")
        );
        return (
          (original.source ?? "null") +
          ":" +
          (original.line ?? "null") +
          ":" +
          (original.name ?? "null")
        );
      }
    );
  }
  symbolicateProfilerMap(mapFile) {
    return fs
      .readFileSync(mapFile, "utf8")
      .split("\n")
      .slice(0, -1)
      .map((line) => {
        const line_list = line.split(" ");
        const trampoline = line_list[0];
        const js_name = line_list[1];
        const offset = parseInt(line_list[2], 10);
        if (!offset) {
          return trampoline + " " + trampoline;
        }
        const original = this.getOriginalPositionFor(
          this.options.inputLineStart,
          offset
        );
        return (
          trampoline +
          " " +
          (original.name || js_name) +
          "::" +
          [original.source, original.line, original.column].join(":")
        );
      })
      .join("\n");
  }
  symbolicateAttribution(obj) {
    const loc = obj.location;
    const line = loc.line != null ? loc.line : this.options.inputLineStart;
    let column = Number(loc.column != null ? loc.column : loc.virtualOffset);
    const file = loc.filename ? this.parseFileName(loc.filename) : null;
    let original = this.getOriginalPositionFor(line, column, file);
    const isBytecodeRange =
      loc.bytecodeSize != null &&
      loc.virtualOffset != null &&
      loc.column == null;
    const virtualOffset = Number(loc.virtualOffset);
    const bytecodeSize = Number(loc.bytecodeSize);
    while (
      isBytecodeRange &&
      original.source == null &&
      ++column < virtualOffset + bytecodeSize
    ) {
      original = this.getOriginalPositionFor(line, column, file);
    }
    obj.location = {
      file: original.source,
      line: original.line,
      column: original.column,
    };
  }
  symbolicateChromeTrace(traceFile, { stdout, stderr }) {
    const content = JSON.parse(fs.readFileSync(traceFile, "utf8"));
    if (content.stackFrames == null) {
      throw new Error("Unable to locate `stackFrames` section in trace.");
    }
    const keys = Object.keys(content.stackFrames);
    stdout.write("Processing " + keys.length + " frames\n");
    keys.forEach((key) => {
      const entry = content.stackFrames[key];
      let line;
      let column;
      let funcLine;
      let funcColumn;
      if (entry.funcVirtAddr != null && entry.offset != null) {
        const funcVirtAddr = parseInt(entry.funcVirtAddr, 10);
        const offsetInFunction = parseInt(entry.offset, 10);
        line = this.options.inputLineStart;
        column = funcVirtAddr + offsetInFunction;
        funcLine = this.options.inputLineStart;
        funcColumn = funcVirtAddr;
      } else if (entry.line != null && entry.column != null) {
        line = entry.line;
        column = entry.column;
        funcLine = entry.funcLine;
        funcColumn = entry.funcColumn;
      } else {
        return;
      }
      const addressOriginal = this.getOriginalPositionDetailsFor(line, column);
      let frameName;
      if (addressOriginal.functionName) {
        frameName = addressOriginal.functionName;
      } else {
        frameName = entry.name;
        if (funcLine != null && funcColumn != null) {
          const funcOriginal = this.getOriginalPositionFor(
            funcLine,
            funcColumn
          );
          if (funcOriginal.name != null) {
            frameName = funcOriginal.name;
          }
        } else {
          (stderr || stdout).write(
            "Warning: no function prolog line/column info; name may be wrong\n"
          );
        }
      }
      entry.name = [
        frameName,
        "(",
        [
          addressOriginal.source ?? "null",
          addressOriginal.line ?? "null",
          addressOriginal.column ?? "null",
        ].join(":"),
        ")",
      ].join("");
    });
    stdout.write("Writing to " + traceFile + "\n");
    fs.writeFileSync(traceFile, JSON.stringify(content));
  }
  getOriginalPositionFor(lineNumber, columnNumber, moduleIds) {
    const position = this.getOriginalPositionDetailsFor(
      lineNumber,
      columnNumber,
      moduleIds
    );
    return {
      line: position.line,
      column: position.column,
      source: position.source,
      name: position.functionName ? position.functionName : position.name,
    };
  }
  symbolicateHermesMinidumpTrace(crashInfo) {
    throw new Error("Not implemented");
  }
  symbolicateHeapSnapshot(snapshotContents) {
    const snapshotData =
      typeof snapshotContents === "string"
        ? JSON.parse(snapshotContents)
        : snapshotContents;
    const processor = new ChromeHeapSnapshotProcessor(snapshotData);
    for (const frame of processor.traceFunctionInfos()) {
      const moduleIds = this.parseFileName(frame.getString("script_name"));
      const generatedLine = frame.getNumber("line");
      const generatedColumn = frame.getNumber("column");
      if (generatedLine === 0 && generatedColumn === 0) {
        continue;
      }
      const {
        line: originalLine,
        column: originalColumn,
        source: originalSource,
        functionName: originalFunctionName,
      } = this.getOriginalPositionDetailsFor(
        frame.getNumber("line") - 1 + this.options.inputLineStart,
        frame.getNumber("column") - 1 + this.options.inputColumnStart,
        moduleIds
      );
      if (originalSource != null) {
        frame.setString("script_name", originalSource);
        if (originalLine != null) {
          frame.setNumber(
            "line",
            originalLine - this.options.outputLineStart + 1
          );
        } else {
          frame.setNumber("line", 0);
        }
        if (originalColumn != null) {
          frame.setNumber(
            "column",
            originalColumn - this.options.outputColumnStart + 1
          );
        } else {
          frame.setNumber("column", 0);
        }
      }
      frame.setString("name", originalFunctionName ?? frame.getString("name"));
    }
    return snapshotData;
  }
  symbolicateHermesCoverageTrace(coverageInfo) {
    const symbolicatedTrace = [];
    const { executedFunctions } = coverageInfo;
    if (executedFunctions != null) {
      for (const stackItem of executedFunctions) {
        const { line, column, SourceURL } = stackItem;
        const generatedLine = line + this.options.inputLineStart;
        const generatedColumn = column + this.options.inputColumnStart;
        const originalPosition = this.getOriginalPositionDetailsFor(
          generatedLine,
          generatedColumn,
          this.parseFileName(SourceURL || "")
        );
        symbolicatedTrace.push(originalPosition);
      }
    }
    return symbolicatedTrace;
  }
  getOriginalPositionDetailsFor(lineNumber, columnNumber, moduleIds) {
    throw new Error("Not implemented");
  }
  parseFileName(str) {
    throw new Error("Not implemented");
  }
}
class SingleMapSymbolicationContext extends SymbolicationContext {
  constructor(SourceMapConsumer, sourceMapContent, options = {}) {
    super(options);
    this._SourceMapConsumer = SourceMapConsumer;
    const sourceMapJson =
      typeof sourceMapContent === "string"
        ? JSON.parse(sourceMapContent.replace(/^\)\]\}'/, ""))
        : sourceMapContent;
    const segments = {
      0: this._initSegment(sourceMapJson),
    };
    if (sourceMapJson.x_facebook_segments) {
      for (const key of Object.keys(sourceMapJson.x_facebook_segments)) {
        const map = sourceMapJson.x_facebook_segments[key];
        segments[key] = this._initSegment(map);
      }
    }
    this._legacyFormat =
      sourceMapJson.x_facebook_segments != null ||
      sourceMapJson.x_facebook_offsets != null;
    this._segments = segments;
  }
  _initSegment(map) {
    const useFunctionNames = this.options.nameSource === "function_names";
    const { _SourceMapConsumer: SourceMapConsumer } = this;
    return {
      get consumer() {
        Object.defineProperty(this, "consumer", {
          value: new SourceMapConsumer(map),
        });
        return this.consumer;
      },
      moduleOffsets: map.x_facebook_offsets || [],
      get sourceFunctionsConsumer() {
        Object.defineProperty(this, "sourceFunctionsConsumer", {
          value: useFunctionNames ? new SourceMetadataMapConsumer(map) : null,
        });
        return this.sourceFunctionsConsumer;
      },
      get googleIgnoreListConsumer() {
        Object.defineProperty(this, "googleIgnoreListConsumer", {
          value: new GoogleIgnoreListConsumer(map),
        });
        return this.googleIgnoreListConsumer;
      },
      hermesOffsets: map.x_hermes_function_offsets,
    };
  }
  symbolicateHermesMinidumpTrace(crashInfo) {
    const symbolicatedTrace = [];
    const { callstack } = crashInfo;
    if (callstack != null) {
      for (const stackItem of callstack) {
        if (stackItem.NativeCode) {
          symbolicatedTrace.push(stackItem);
        } else {
          const {
            CJSModuleOffset,
            SegmentID,
            SourceURL,
            FunctionID,
            ByteCodeOffset: localOffset,
          } = stackItem;
          const cjsModuleOffsetOrSegmentID = nullthrows(
            CJSModuleOffset ?? SegmentID,
            "Either CJSModuleOffset or SegmentID must be specified in the Hermes stack frame"
          );
          const moduleInformation = this.parseFileName(SourceURL);
          const generatedLine =
            cjsModuleOffsetOrSegmentID + this.options.inputLineStart;
          const segment =
            this._segments[moduleInformation.segmentId.toString()];
          const hermesOffsets = segment?.hermesOffsets;
          if (!hermesOffsets) {
            symbolicatedTrace.push({
              line: null,
              column: null,
              source: null,
              functionName: null,
              name: null,
              isIgnored: false,
            });
          } else {
            const segmentOffsets =
              hermesOffsets[Number(cjsModuleOffsetOrSegmentID)];
            const generatedColumn =
              segmentOffsets[FunctionID] +
              localOffset +
              this.options.inputColumnStart;
            const originalPosition = this.getOriginalPositionDetailsFor(
              generatedLine,
              generatedColumn,
              moduleInformation
            );
            symbolicatedTrace.push(originalPosition);
          }
        }
      }
    }
    return symbolicatedTrace;
  }
  symbolicateHermesCoverageTrace(coverageInfo) {
    const symbolicatedTrace = [];
    const { executedFunctions } = coverageInfo;
    if (executedFunctions != null) {
      for (const stackItem of executedFunctions) {
        const { line, column, SourceURL } = stackItem;
        const generatedLine = line + this.options.inputLineStart;
        const generatedColumn = column + this.options.inputColumnStart;
        const originalPosition = this.getOriginalPositionDetailsFor(
          generatedLine,
          generatedColumn,
          this.parseFileName(SourceURL || "")
        );
        symbolicatedTrace.push(originalPosition);
      }
    }
    return symbolicatedTrace;
  }
  getOriginalPositionDetailsFor(lineNumber, columnNumber, moduleIds) {
    lineNumber =
      lineNumber != null
        ? lineNumber - this.options.inputLineStart + 1
        : lineNumber;
    columnNumber =
      columnNumber != null
        ? columnNumber - this.options.inputColumnStart + 0
        : columnNumber;
    if (!moduleIds) {
      moduleIds = UNKNOWN_MODULE_IDS;
    }
    let moduleLineOffset = 0;
    const metadata = this._segments[moduleIds.segmentId + ""];
    const { localId } = moduleIds;
    if (localId != null) {
      const { moduleOffsets } = metadata;
      if (!moduleOffsets) {
        throw new Error(
          "Module ID given for a source map that does not have " +
            "an x_facebook_offsets field"
        );
      }
      if (moduleOffsets[localId] == null) {
        throw new Error("Unknown module ID: " + localId);
      }
      moduleLineOffset = moduleOffsets[localId];
    }
    const original = metadata.consumer.originalPositionFor({
      line: Number(lineNumber) + moduleLineOffset,
      column: Number(columnNumber),
    });
    if (metadata.sourceFunctionsConsumer) {
      original.functionName =
        metadata.sourceFunctionsConsumer.functionNameFor(original) || null;
    } else {
      original.functionName = null;
    }
    original.isIgnored = metadata.googleIgnoreListConsumer.isIgnored(original);
    return {
      ...original,
      line:
        original.line != null
          ? original.line - 1 + this.options.outputLineStart
          : original.line,
      column:
        original.column != null
          ? original.column - 0 + this.options.outputColumnStart
          : original.column,
    };
  }
  parseFileName(str) {
    if (this._legacyFormat) {
      return parseSingleMapFileName(str);
    }
    return UNKNOWN_MODULE_IDS;
  }
}
class DirectorySymbolicationContext extends SymbolicationContext {
  constructor(SourceMapConsumer, rootDir, options = {}) {
    super(options);
    this._fileMaps = new Map();
    this._rootDir = rootDir;
    this._SourceMapConsumer = SourceMapConsumer;
  }
  _loadMap(mapFilename) {
    invariant(
      fs.existsSync(mapFilename),
      `Could not read source map from '${mapFilename}'`
    );
    let fileMap = this._fileMaps.get(mapFilename);
    if (fileMap == null) {
      fileMap = new SingleMapSymbolicationContext(
        this._SourceMapConsumer,
        fs.readFileSync(mapFilename, "utf8"),
        this.options
      );
      this._fileMaps.set(mapFilename, fileMap);
    }
    return fileMap;
  }
  getOriginalPositionDetailsFor(lineNumber, columnNumber, filename) {
    invariant(
      filename != null,
      "filename is required for DirectorySymbolicationContext"
    );
    let mapFilename;
    const relativeFilename = path.relative(
      this._rootDir,
      path.resolve(this._rootDir, filename)
    );
    if (!relativeFilename.startsWith("..")) {
      mapFilename = path.join(this._rootDir, relativeFilename + ".map");
    }
    if (mapFilename == null || !fs.existsSync(mapFilename)) {
      lineNumber =
        lineNumber != null
          ? lineNumber -
            this.options.inputLineStart +
            this.options.outputLineStart
          : lineNumber;
      columnNumber =
        columnNumber != null
          ? columnNumber -
            this.options.inputColumnStart +
            this.options.outputColumnStart
          : columnNumber;
      return {
        line: lineNumber,
        column: columnNumber,
        source: filename,
        name: null,
        functionName: null,
        isIgnored: false,
      };
    }
    return this._loadMap(mapFilename).getOriginalPositionDetailsFor(
      lineNumber,
      columnNumber
    );
  }
  parseFileName(str) {
    return str;
  }
}
function parseSingleMapFileName(str) {
  const modMatch = str.match(/^(\d+).js$/);
  if (modMatch != null) {
    return {
      segmentId: 0,
      localId: Number(modMatch[1]),
    };
  }
  const segMatch = str.match(/^seg-(\d+)(?:_(\d+))?.js$/);
  if (segMatch != null) {
    return {
      segmentId: Number(segMatch[1]),
      localId: segMatch[2] ? Number(segMatch[2]) : null,
    };
  }
  return UNKNOWN_MODULE_IDS;
}
function createContext(SourceMapConsumer, sourceMapContent, options = {}) {
  return new SingleMapSymbolicationContext(
    SourceMapConsumer,
    sourceMapContent,
    options
  );
}
function unstable_createDirectoryContext(
  SourceMapConsumer,
  rootDir,
  options = {}
) {
  return new DirectorySymbolicationContext(SourceMapConsumer, rootDir, options);
}
function getOriginalPositionFor(lineNumber, columnNumber, moduleIds, context) {
  return context.getOriginalPositionFor(lineNumber, columnNumber, moduleIds);
}
function symbolicate(stackTrace, context) {
  return context.symbolicate(stackTrace);
}
function symbolicateProfilerMap(mapFile, context) {
  return context.symbolicateProfilerMap(mapFile);
}
function symbolicateAttribution(obj, context) {
  context.symbolicateAttribution(obj);
}
function symbolicateChromeTrace(traceFile, { stdout, stderr }, context) {
  return context.symbolicateChromeTrace(traceFile, {
    stdout,
    stderr,
  });
}
module.exports = {
  createContext,
  unstable_createDirectoryContext,
  getOriginalPositionFor,
  parseFileName: parseSingleMapFileName,
  symbolicate,
  symbolicateProfilerMap,
  symbolicateAttribution,
  symbolicateChromeTrace,
  SourceMetadataMapConsumer,
};
