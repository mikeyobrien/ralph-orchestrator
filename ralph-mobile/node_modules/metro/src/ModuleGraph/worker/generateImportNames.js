"use strict";

const traverse = require("@babel/traverse").default;
const nullthrows = require("nullthrows");
function generateImportNames(ast) {
  let importDefault;
  let importAll;
  traverse(ast, {
    Program(path) {
      importAll = path.scope.generateUid("$$_IMPORT_ALL");
      importDefault = path.scope.generateUid("$$_IMPORT_DEFAULT");
      path.stop();
    },
  });
  return {
    importAll: nullthrows(importAll),
    importDefault: nullthrows(importDefault),
  };
}
module.exports = generateImportNames;
