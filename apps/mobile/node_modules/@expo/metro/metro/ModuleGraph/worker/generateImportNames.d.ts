import type * as _babel_types from "@babel/types";
/**
 * Select unused names for "metroImportDefault" and "metroImportAll", by
 * calling "generateUid".
 */
declare function generateImportNames(ast: _babel_types.Node): {
  importAll: string;
  importDefault: string;
};
export default generateImportNames;