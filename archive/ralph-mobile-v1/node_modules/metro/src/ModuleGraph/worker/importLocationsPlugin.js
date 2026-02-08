"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.importLocationsPlugin = importLocationsPlugin;
exports.locToKey = locToKey;
function importLocationsPlugin({ types: t }) {
  return {
    visitor: {
      ImportDeclaration(path, { importDeclarationLocs }) {
        if (path.node.importKind !== "type" && path.node.loc != null) {
          importDeclarationLocs.add(locToKey(path.node.loc));
        }
      },
      ExportDeclaration(path, { importDeclarationLocs }) {
        if (
          path.node.source != null &&
          path.node.exportKind !== "type" &&
          path.node.loc != null
        ) {
          importDeclarationLocs.add(locToKey(path.node.loc));
        }
      },
      Program(path, state) {
        state.importDeclarationLocs = new Set();
        const metroMetadata = state.file.metadata;
        if (!metroMetadata.metro) {
          metroMetadata.metro = {
            unstable_importDeclarationLocs: state.importDeclarationLocs,
          };
        } else {
          metroMetadata.metro.unstable_importDeclarationLocs =
            state.importDeclarationLocs;
        }
      },
    },
  };
}
const MISSING_LOC = {
  line: -1,
  column: -1,
};
function locToKey(loc) {
  const { start = MISSING_LOC, end = MISSING_LOC } = loc;
  return `${start.line},${start.column}:${end.line},${end.column}`;
}
