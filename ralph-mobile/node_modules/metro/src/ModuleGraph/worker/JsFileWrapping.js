"use strict";

var _template = _interopRequireDefault(require("@babel/template"));
var _traverse = _interopRequireDefault(require("@babel/traverse"));
var t = _interopRequireWildcard(require("@babel/types"));
var _invariant = _interopRequireDefault(require("invariant"));
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
  return (n.default = e), t && t.set(e, n), n;
}
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const WRAP_NAME = "$$_REQUIRE";
const IIFE_PARAM = _template.default.expression(
  "typeof globalThis !== 'undefined' ? globalThis : typeof global !== 'undefined' ? global : typeof window !== 'undefined' ? window : this"
);
function wrapModule(
  fileAst,
  importDefaultName,
  importAllName,
  dependencyMapName,
  globalPrefix,
  skipRequireRename
) {
  const params = buildParameters(
    importDefaultName,
    importAllName,
    dependencyMapName
  );
  const factory = functionFromProgram(fileAst.program, params);
  const def = t.callExpression(t.identifier(`${globalPrefix}__d`), [factory]);
  const ast = t.file(t.program([t.expressionStatement(def)]));
  const requireName = skipRequireRename ? "require" : renameRequires(ast);
  return {
    ast,
    requireName,
  };
}
function wrapPolyfill(fileAst) {
  const factory = functionFromProgram(fileAst.program, ["global"]);
  const iife = t.callExpression(factory, [IIFE_PARAM()]);
  return t.file(t.program([t.expressionStatement(iife)]));
}
function jsonToCommonJS(source) {
  return `module.exports = ${source};`;
}
function wrapJson(source, globalPrefix) {
  const moduleFactoryParameters = buildParameters(
    "_importDefaultUnused",
    "_importAllUnused",
    "_dependencyMapUnused"
  );
  return [
    `${globalPrefix}__d(function(${moduleFactoryParameters.join(", ")}) {`,
    `  ${jsonToCommonJS(source)}`,
    "});",
  ].join("\n");
}
function functionFromProgram(program, parameters) {
  return t.functionExpression(
    undefined,
    parameters.map(makeIdentifier),
    t.blockStatement(program.body, program.directives)
  );
}
function makeIdentifier(name) {
  return t.identifier(name);
}
function buildParameters(importDefaultName, importAllName, dependencyMapName) {
  return [
    "global",
    "require",
    importDefaultName,
    importAllName,
    "module",
    "exports",
    dependencyMapName,
  ];
}
function renameRequires(ast) {
  let newRequireName = WRAP_NAME;
  (0, _traverse.default)(ast, {
    Program(path) {
      const body = path.get("body.0.expression.arguments.0.body");
      (0, _invariant.default)(
        !Array.isArray(body),
        "metro: Expected `body` to be a single path."
      );
      newRequireName = body.scope.generateUid(WRAP_NAME);
      body.scope.rename("require", newRequireName);
    },
  });
  return newRequireName;
}
module.exports = {
  WRAP_NAME,
  wrapJson,
  jsonToCommonJS,
  wrapModule,
  wrapPolyfill,
};
