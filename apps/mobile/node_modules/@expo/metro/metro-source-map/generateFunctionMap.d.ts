import type { FBSourceFunctionMap } from "./source-map";
import type { PluginObj } from "@babel/core";
import type { Node } from "@babel/types";
export interface Position {
  line: number;
  column: number;
}
export interface RangeMapping {
  name: string;
  start: Position;
}
export interface Context {
  filename?: null | undefined | string;
}
declare function generateFunctionMap(ast: Node, context?: Context): FBSourceFunctionMap;
declare function generateFunctionMappingsArray(ast: Node, context?: Context): ReadonlyArray<RangeMapping>;
declare function functionMapBabelPlugin(): PluginObj;
export { functionMapBabelPlugin, generateFunctionMap, generateFunctionMappingsArray };