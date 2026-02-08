"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = generateImportNames;
var _traverse = _interopRequireDefault(require("@babel/traverse"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function generateImportNames(ast) {
  let importDefault;
  let importAll;
  (0, _traverse.default)(ast, {
    Program(path) {
      importAll = path.scope.generateUid("$$_IMPORT_ALL");
      importDefault = path.scope.generateUid("$$_IMPORT_DEFAULT");
      path.stop();
    },
  });
  return {
    importAll: (0, _nullthrows.default)(importAll),
    importDefault: (0, _nullthrows.default)(importDefault),
  };
}
