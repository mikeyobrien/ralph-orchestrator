"use strict";

const importMap = new Map([["ReactNative", "react-native"]]);
function createInlinePlatformChecks(t, requireName = "require") {
  const {
    isIdentifier,
    isStringLiteral,
    isNumericLiteral,
    isMemberExpression,
    isCallExpression,
  } = t;
  const isPlatformNode = (node, scope, isWrappedModule) =>
    isPlatformOS(node, scope, isWrappedModule) ||
    isReactPlatformOS(node, scope, isWrappedModule);
  const isPlatformSelectNode = (node, scope, isWrappedModule) =>
    isPlatformSelect(node, scope, isWrappedModule) ||
    isReactPlatformSelect(node, scope, isWrappedModule);
  const isPlatformOS = (node, scope, isWrappedModule) =>
    isIdentifier(node.property, {
      name: "OS",
    }) &&
    isImportOrGlobal(
      node.object,
      scope,
      [
        {
          name: "Platform",
        },
      ],
      isWrappedModule
    );
  const isReactPlatformOS = (node, scope, isWrappedModule) =>
    isIdentifier(node.property, {
      name: "OS",
    }) &&
    isMemberExpression(node.object) &&
    isIdentifier(node.object.property, {
      name: "Platform",
    }) &&
    isImportOrGlobal(
      node.object.object,
      scope,
      [
        {
          name: "React",
        },
        {
          name: "ReactNative",
        },
      ],
      isWrappedModule
    );
  const isPlatformSelect = (node, scope, isWrappedModule) =>
    isMemberExpression(node.callee) &&
    isIdentifier(node.callee.property, {
      name: "select",
    }) &&
    isImportOrGlobal(
      node.callee.object,
      scope,
      [
        {
          name: "Platform",
        },
      ],
      isWrappedModule
    );
  const isReactPlatformSelect = (node, scope, isWrappedModule) =>
    isMemberExpression(node.callee) &&
    isIdentifier(node.callee.property, {
      name: "select",
    }) &&
    isMemberExpression(node.callee.object) &&
    isIdentifier(node.callee.object.property, {
      name: "Platform",
    }) &&
    isImportOrGlobal(
      node.callee.object.object,
      scope,
      [
        {
          name: "React",
        },
        {
          name: "ReactNative",
        },
      ],
      isWrappedModule
    );
  const isRequireCall = (node, dependencyId, scope) =>
    isCallExpression(node) &&
    isIdentifier(node.callee, {
      name: requireName,
    }) &&
    checkRequireArgs(node.arguments, dependencyId);
  const isImport = (node, scope, patterns) =>
    patterns.some((pattern) => {
      const importName = importMap.get(pattern.name) || pattern.name;
      return isRequireCall(node, importName, scope);
    });
  const isImportOrGlobal = (node, scope, patterns, isWrappedModule) => {
    const identifier = patterns.find((pattern) => isIdentifier(node, pattern));
    return (
      (!!identifier &&
        isToplevelBinding(
          scope.getBinding(identifier.name),
          isWrappedModule
        )) ||
      isImport(node, scope, patterns)
    );
  };
  const checkRequireArgs = (args, dependencyId) => {
    const pattern = t.stringLiteral(dependencyId);
    return (
      isStringLiteral(args[0], pattern) ||
      (isMemberExpression(args[0]) &&
        isNumericLiteral(args[0].property) &&
        isStringLiteral(args[1], pattern))
    );
  };
  const isToplevelBinding = (binding, isWrappedModule) =>
    !binding ||
    !binding.scope.parent ||
    (isWrappedModule && !binding.scope.parent.parent);
  return {
    isPlatformNode,
    isPlatformSelectNode,
  };
}
module.exports = createInlinePlatformChecks;
