"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = normalizePseudoglobals;
var _traverse = _interopRequireDefault(require("@babel/traverse"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function normalizePseudoglobals(ast, options) {
  const reservedNames = new Set(options?.reservedNames ?? []);
  const renamedParamNames = [];
  (0, _traverse.default)(ast, {
    Program(path) {
      const params = path.get("body.0.expression.arguments.0.params");
      const body = path.get("body.0.expression.arguments.0.body");
      if (!body || Array.isArray(body) || !Array.isArray(params)) {
        path.stop();
        return;
      }
      const pseudoglobals = params
        .map((path) => path.node.name)
        .filter((name) => !reservedNames.has(name));
      const usedShortNames = new Set();
      const namePairs = pseudoglobals.map((fullName) => [
        fullName,
        getShortName(fullName, usedShortNames),
      ]);
      for (const [fullName, shortName] of namePairs) {
        if (reservedNames.has(shortName)) {
          throw new ReferenceError(
            "Could not reserve the identifier " +
              shortName +
              " because it is the short name for " +
              fullName,
          );
        }
        renamedParamNames.push(rename(fullName, shortName, body.scope));
      }
      path.stop();
    },
  });
  return renamedParamNames;
}
function getShortName(fullName, usedNames) {
  const regexp = /^[^A-Za-z]*([A-Za-z])|([A-Z])[a-z]|([A-Z])[A-Z]+$/g;
  let match;
  while ((match = regexp.exec(fullName))) {
    const name = (match[1] || match[2] || match[3] || "").toLowerCase();
    if (!name) {
      throw new ReferenceError(
        "Could not identify any valid name for " + fullName,
      );
    }
    if (!usedNames.has(name)) {
      usedNames.add(name);
      return name;
    }
  }
  throw new ReferenceError(
    `Unable to determine short name for ${fullName}. The variables are not unique: ${Array.from(usedNames).join(", ")}`,
  );
}
function rename(fullName, shortName, scope) {
  let unusedName = shortName;
  if (
    scope.hasLabel(shortName) ||
    scope.hasBinding(shortName) ||
    scope.hasGlobal(shortName) ||
    scope.hasReference(shortName)
  ) {
    unusedName = scope.generateUid(shortName);
    const programScope = scope.getProgramParent();
    (0, _nullthrows.default)(programScope.references)[shortName] = true;
    (0, _nullthrows.default)(programScope.uids)[shortName] = true;
  }
  scope.rename(fullName, unusedName);
  return unusedName;
}
