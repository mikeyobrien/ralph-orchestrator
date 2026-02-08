import type * as _nodeStream from "node:stream";
import type { ChromeHeapSnapshot } from "./ChromeHeapSnapshot";
import type { HermesFunctionOffsets, MixedSourceMap } from "../metro-source-map";
import GoogleIgnoreListConsumer from "./GoogleIgnoreListConsumer";
import SourceMetadataMapConsumer from "./SourceMetadataMapConsumer";
import { type SourceMapConsumer as $$IMPORT_TYPEOF_1$$ } from "source-map";
type SourceMapConsumer = typeof $$IMPORT_TYPEOF_1$$;
export interface SingleMapModuleIds {
  segmentId: number;
  localId?: null | number;
}
export interface ContextOptionsInput {
  readonly nameSource?: "function_names" | "identifier_names";
  readonly inputLineStart?: number;
  readonly inputColumnStart?: number;
  readonly outputLineStart?: number;
  readonly outputColumnStart?: number;
}
export interface _SizeAttributionMap_location {
  file?: null | string;
  filename?: string;
  bytecodeSize?: number;
  virtualOffset?: number;
  line?: null | number;
  column?: null | number;
}
export interface SizeAttributionMap {
  location: _SizeAttributionMap_location;
}
export interface HermesMinidumpCrashInfo {
  readonly callstack: ReadonlyArray<HermesMinidumpStackFrame | NativeCodeStackFrame>;
}
export interface HermesMinidumpStackFrame {
  readonly ByteCodeOffset: number;
  readonly FunctionID: number;
  readonly CJSModuleOffset?: number;
  readonly SegmentID?: number;
  readonly SourceURL: string;
  readonly StackFrameRegOffs: string;
  readonly SourceLocation?: string;
}
export interface HermesCoverageInfo {
  readonly executedFunctions: ReadonlyArray<HermesCoverageStackFrame>;
}
export interface HermesCoverageStackFrame {
  readonly line: number;
  readonly column: number;
  readonly SourceURL?: null | string;
}
export interface NativeCodeStackFrame {
  readonly NativeCode: true;
  readonly StackFrameRegOffs: string;
}
type SymbolicatedStackTrace = ReadonlyArray<SymbolicatedStackFrame | NativeCodeStackFrame>;
export interface SymbolicatedStackFrame {
  readonly line?: null | number;
  readonly column?: null | number;
  readonly source?: null | string;
  readonly functionName?: null | string;
  readonly name?: null | string;
  readonly isIgnored: boolean;
}
declare class SymbolicationContext<ModuleIdsT> {
  readonly options: {
    readonly nameSource?: "function_names" | "identifier_names";
    readonly inputLineStart: number;
    readonly inputColumnStart: number;
    readonly outputLineStart: number;
    readonly outputColumnStart: number;
  };
  constructor(options: ContextOptionsInput);
  symbolicate(stackTrace: string): string;
  symbolicateProfilerMap(mapFile: string): string;
  symbolicateAttribution(obj: SizeAttributionMap): void;
  symbolicateChromeTrace(traceFile: string, $$PARAM_1$$: {
    stdout: _nodeStream.Writable;
    stderr: _nodeStream.Writable;
  }): void;
  getOriginalPositionFor(lineNumber: null | undefined | number, columnNumber: null | undefined | number, moduleIds: null | undefined | ModuleIdsT): {
    line?: null | number;
    column?: null | number;
    source?: null | string;
    name?: null | string;
  };
  symbolicateHermesMinidumpTrace(crashInfo: HermesMinidumpCrashInfo): SymbolicatedStackTrace;
  symbolicateHeapSnapshot(snapshotContents: string | ChromeHeapSnapshot): ChromeHeapSnapshot;
  symbolicateHermesCoverageTrace(coverageInfo: HermesCoverageInfo): SymbolicatedStackTrace;
  getOriginalPositionDetailsFor(lineNumber: null | undefined | number, columnNumber: null | undefined | number, moduleIds: null | undefined | ModuleIdsT): SymbolicatedStackFrame;
  parseFileName(str: string): ModuleIdsT;
}
declare class SingleMapSymbolicationContext extends SymbolicationContext<SingleMapModuleIds> {
  readonly _segments: {
    readonly [id: string]: {
      readonly consumer: SourceMapConsumer;
      readonly moduleOffsets: ReadonlyArray<number>;
      readonly sourceFunctionsConsumer?: null | SourceMetadataMapConsumer;
      readonly hermesOffsets?: null | HermesFunctionOffsets;
      readonly googleIgnoreListConsumer: GoogleIgnoreListConsumer;
    };
  };
  readonly _legacyFormat: boolean;
  readonly _SourceMapConsumer: SourceMapConsumer;
  constructor(SourceMapConsumer: SourceMapConsumer, sourceMapContent: string | MixedSourceMap, options: ContextOptionsInput);
  _initSegment(map: MixedSourceMap): void;
  symbolicateHermesMinidumpTrace(crashInfo: HermesMinidumpCrashInfo): SymbolicatedStackTrace;
  symbolicateHermesCoverageTrace(coverageInfo: HermesCoverageInfo): SymbolicatedStackTrace;
  getOriginalPositionDetailsFor(lineNumber: null | undefined | number, columnNumber: null | undefined | number, moduleIds: null | undefined | SingleMapModuleIds): SymbolicatedStackFrame;
  parseFileName(str: string): SingleMapModuleIds;
}
declare class DirectorySymbolicationContext extends SymbolicationContext<string> {
  readonly _fileMaps: Map<string, SingleMapSymbolicationContext>;
  readonly _rootDir: string;
  readonly _SourceMapConsumer: SourceMapConsumer;
  constructor(SourceMapConsumer: SourceMapConsumer, rootDir: string, options: ContextOptionsInput);
  _loadMap(mapFilename: string): SingleMapSymbolicationContext;
  getOriginalPositionDetailsFor(lineNumber: null | undefined | number, columnNumber: null | undefined | number, filename: null | undefined | string): SymbolicatedStackFrame;
  parseFileName(str: string): string;
}
declare function parseSingleMapFileName(str: string): SingleMapModuleIds;
declare function createContext(SourceMapConsumer: SourceMapConsumer, sourceMapContent: string | MixedSourceMap, options?: ContextOptionsInput): SingleMapSymbolicationContext;
declare function unstable_createDirectoryContext(SourceMapConsumer: SourceMapConsumer, rootDir: string, options?: ContextOptionsInput): DirectorySymbolicationContext;
declare function getOriginalPositionFor<ModuleIdsT>(lineNumber: null | undefined | number, columnNumber: null | undefined | number, moduleIds: null | undefined | ModuleIdsT, context: SymbolicationContext<ModuleIdsT>): {
  line?: null | number;
  column?: null | number;
  source?: null | string;
  name?: null | string;
};
declare function symbolicate<ModuleIdsT>(stackTrace: string, context: SymbolicationContext<ModuleIdsT>): string;
declare function symbolicateProfilerMap<ModuleIdsT>(mapFile: string, context: SymbolicationContext<ModuleIdsT>): string;
declare function symbolicateAttribution<ModuleIdsT>(obj: SizeAttributionMap, context: SymbolicationContext<ModuleIdsT>): void;
declare function symbolicateChromeTrace<ModuleIdsT>(traceFile: string, $$PARAM_1$$: {
  stdout: _nodeStream.Writable;
  stderr: _nodeStream.Writable;
}, context: SymbolicationContext<ModuleIdsT>): void;
export { createContext, unstable_createDirectoryContext, getOriginalPositionFor, parseSingleMapFileName as parseFileName, symbolicate, symbolicateProfilerMap, symbolicateAttribution, symbolicateChromeTrace, SourceMetadataMapConsumer };