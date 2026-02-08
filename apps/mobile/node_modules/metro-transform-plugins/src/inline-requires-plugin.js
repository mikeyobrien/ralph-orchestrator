"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _default = ({ types: t, traverse }) => ({
  name: "inline-requires",
  visitor: {
    Program: {
      enter() {},
      exit(path, state) {
        const ignoredRequires = new Set();
        const inlineableCalls = new Set(["require"]);
        const nonMemoizedModules = new Set();
        let memoizeCalls = false;
        const opts = state.opts;
        if (opts != null) {
          opts.ignoredRequires?.forEach((name) => ignoredRequires.add(name));
          opts.inlineableCalls?.forEach((name) => inlineableCalls.add(name));
          opts.nonMemoizedModules?.forEach((name) =>
            nonMemoizedModules.add(name),
          );
          memoizeCalls = opts.memoizeCalls ?? false;
        }
        const programNode = path.scope.block;
        if (programNode.type !== "Program") {
          return;
        }
        path.scope.crawl();
        path.traverse(
          {
            CallExpression(path, state) {
              const parseResult =
                parseInlineableAlias(path, state) ||
                parseInlineableMemberAlias(path, state);
              if (parseResult == null) {
                return;
              }
              const { declarationPath, moduleName, requireFnName } =
                parseResult;
              const maybeInit = declarationPath.node.init;
              const name = declarationPath.node.id
                ? declarationPath.node.id.name
                : null;
              const binding =
                name == null ? null : declarationPath.scope.getBinding(name);
              if (
                maybeInit == null ||
                !t.isExpression(maybeInit) ||
                binding == null ||
                binding.constantViolations.length > 0
              ) {
                return;
              }
              const init = maybeInit;
              const initPath = declarationPath.get("init");
              if (Array.isArray(initPath)) {
                return;
              }
              const initLoc = getNearestLocFromPath(initPath);
              deleteLocation(init);
              traverse(init, {
                noScope: true,
                enter: (path) => deleteLocation(path.node),
              });
              let thrown = false;
              const memoVarName = parseResult.identifierName;
              let hasMemoVar = false;
              if (
                memoizeCalls &&
                binding.referencePaths.length > 0 &&
                !nonMemoizedModules.has(moduleName)
              ) {
                const varInitStmt = t.variableDeclaration("var", [
                  t.variableDeclarator(t.identifier(memoVarName)),
                ]);
                declarationPath.remove();
                hasMemoVar = addStmtToBlock(programNode, varInitStmt, 0);
              }
              function getMemoOrCallExpr() {
                const refExpr = t.cloneDeep(init);
                refExpr.METRO_INLINE_REQUIRES_INIT_LOC = initLoc;
                return t.logicalExpression(
                  "||",
                  t.identifier(memoVarName),
                  t.assignmentExpression(
                    "=",
                    t.identifier(memoVarName),
                    refExpr,
                  ),
                );
              }
              const scopesWithInlinedRequire = new Set();
              for (const referencePath of binding.referencePaths) {
                excludeMemberAssignment(moduleName, referencePath, state);
                try {
                  referencePath.scope.rename(requireFnName);
                  if (hasMemoVar) {
                    referencePath.scope.rename(memoVarName);
                    if (!isDirectlyEnclosedByBlock(t, referencePath)) {
                      referencePath.replaceWith(getMemoOrCallExpr());
                      continue;
                    }
                    if (scopesWithInlinedRequire.has(referencePath.scope)) {
                      referencePath.replaceWith(t.identifier(memoVarName));
                    } else {
                      referencePath.replaceWith(getMemoOrCallExpr());
                      scopesWithInlinedRequire.add(referencePath.scope);
                    }
                  } else {
                    const refExpr = t.cloneDeep(init);
                    refExpr.METRO_INLINE_REQUIRES_INIT_LOC = initLoc;
                    referencePath.replaceWith(refExpr);
                  }
                } catch (error) {
                  thrown = true;
                }
              }
              if (!thrown && declarationPath.node != null) {
                declarationPath.remove();
              }
            },
          },
          {
            ignoredRequires,
            inlineableCalls,
            membersAssigned: new Map(),
          },
        );
      },
    },
  },
});
exports.default = _default;
function excludeMemberAssignment(moduleName, referencePath, state) {
  const assignment = referencePath.parentPath?.parent;
  if (assignment?.type !== "AssignmentExpression") {
    return;
  }
  const left = assignment.left;
  if (left.type !== "MemberExpression" || left.object !== referencePath.node) {
    return;
  }
  const memberPropertyName = getMemberPropertyName(left);
  if (memberPropertyName == null) {
    return;
  }
  let membersAssigned = state.membersAssigned.get(moduleName);
  if (membersAssigned == null) {
    membersAssigned = new Set();
    state.membersAssigned.set(moduleName, membersAssigned);
  }
  membersAssigned.add(memberPropertyName);
}
function isExcludedMemberAssignment(moduleName, memberPropertyName, state) {
  const excludedAliases = state.membersAssigned.get(moduleName);
  return excludedAliases != null && excludedAliases.has(memberPropertyName);
}
function getMemberPropertyName(node) {
  if (node.property.type === "Identifier") {
    return node.property.name;
  }
  if (node.property.type === "StringLiteral") {
    return node.property.value;
  }
  return null;
}
function deleteLocation(node) {
  delete node.start;
  delete node.end;
  delete node.loc;
}
function parseInlineableAlias(path, state) {
  const module = getInlineableModule(path, state);
  if (module == null) {
    return null;
  }
  const { moduleName, requireFnName } = module;
  const parentPath = path.parentPath;
  if (parentPath == null) {
    return null;
  }
  const grandParentPath = parentPath.parentPath;
  if (grandParentPath == null) {
    return null;
  }
  if (path.parent.type !== "VariableDeclarator") {
    return null;
  }
  const variableDeclarator = path.parent;
  if (variableDeclarator.id.type !== "Identifier") {
    return null;
  }
  const identifier = variableDeclarator.id;
  const isValid =
    parentPath.parent.type === "VariableDeclaration" &&
    grandParentPath.parent.type === "Program";
  return !isValid || parentPath.node == null
    ? null
    : {
        declarationPath: parentPath,
        moduleName,
        requireFnName,
        identifierName: identifier.name,
      };
}
function parseInlineableMemberAlias(path, state) {
  const module = getInlineableModule(path, state);
  if (module == null) {
    return null;
  }
  const { moduleName, requireFnName } = module;
  const parent = path.parent;
  const parentPath = path.parentPath;
  if (parentPath == null) {
    return null;
  }
  const grandParentPath = parentPath.parentPath;
  if (grandParentPath == null) {
    return null;
  }
  if (parent.type !== "MemberExpression") {
    return null;
  }
  const memberExpression = parent;
  if (parentPath.parent.type !== "VariableDeclarator") {
    return null;
  }
  const variableDeclarator = parentPath.parent;
  if (variableDeclarator.id.type !== "Identifier") {
    return null;
  }
  const identifier = variableDeclarator.id;
  if (
    grandParentPath.parent.type !== "VariableDeclaration" ||
    grandParentPath.parentPath?.parent.type !== "Program" ||
    grandParentPath.node == null
  ) {
    return null;
  }
  const memberPropertyName = getMemberPropertyName(memberExpression);
  return memberPropertyName == null ||
    isExcludedMemberAssignment(moduleName, memberPropertyName, state)
    ? null
    : {
        declarationPath: grandParentPath,
        moduleName,
        requireFnName,
        identifierName: identifier.name,
      };
}
function getInlineableModule(path, state) {
  const node = path.node;
  const isInlineable =
    node.type === "CallExpression" &&
    node.callee.type === "Identifier" &&
    state.inlineableCalls.has(node.callee.name) &&
    node["arguments"].length >= 1;
  if (!isInlineable) {
    return null;
  }
  let moduleName =
    node["arguments"][0].type === "StringLiteral"
      ? node["arguments"][0].value
      : null;
  if (moduleName == null) {
    const callNode = node["arguments"][0];
    if (
      callNode.type === "CallExpression" &&
      callNode.callee.type === "MemberExpression" &&
      callNode.callee.object.type === "Identifier"
    ) {
      const callee = callNode.callee;
      moduleName =
        callee.object.type === "Identifier" &&
        state.inlineableCalls.has(callee.object.name) &&
        callee.property.type === "Identifier" &&
        callee.property.name === "resolve" &&
        callNode["arguments"].length >= 1 &&
        callNode["arguments"][0].type === "StringLiteral"
          ? callNode["arguments"][0].value
          : null;
    }
  }
  const fnName = node.callee.name;
  if (fnName == null) {
    return null;
  }
  const isRequireInScope = path.scope.getBinding(fnName) != null;
  return moduleName == null ||
    state.ignoredRequires.has(moduleName) ||
    moduleName.startsWith("@babel/runtime/") ||
    isRequireInScope
    ? null
    : {
        moduleName,
        requireFnName: fnName,
      };
}
function getNearestLocFromPath(path) {
  let current = path;
  while (current && !current.node.loc) {
    current = current.parentPath;
  }
  return current?.node.loc;
}
function isBranch(t, node) {
  return (
    t.isIfStatement(node) ||
    t.isLogicalExpression(node) ||
    t.isConditionalExpression(node) ||
    t.isSwitchStatement(node) ||
    t.isSwitchCase(node) ||
    t.isForStatement(node) ||
    t.isForInStatement(node) ||
    t.isForOfStatement(node) ||
    t.isWhileStatement(node)
  );
}
function isDirectlyEnclosedByBlock(t, path) {
  let curPath = path;
  while (curPath) {
    if (isBranch(t, curPath.node)) {
      return false;
    }
    if (t.isBlockStatement(curPath.node)) {
      return true;
    }
    curPath = curPath.parentPath;
  }
  return true;
}
function addStmtToBlock(block, stmt, idx) {
  const scopeBody = block.body;
  if (Array.isArray(scopeBody)) {
    scopeBody.splice(idx, 0, stmt);
    return true;
  } else if (scopeBody && Array.isArray(scopeBody.body)) {
    scopeBody.body.splice(idx, 0, stmt);
    return true;
  } else {
    return false;
  }
}
