"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = inlinePlugin;
var _createInlinePlatformChecks = _interopRequireDefault(
  require("./utils/createInlinePlatformChecks"),
);
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const env = {
  name: "env",
};
const nodeEnv = {
  name: "NODE_ENV",
};
const processId = {
  name: "process",
};
const dev = {
  name: "__DEV__",
};
function inlinePlugin({ types: t }, options) {
  const {
    isAssignmentExpression,
    isIdentifier,
    isMemberExpression,
    isObjectExpression,
    isObjectMethod,
    isObjectProperty,
    isSpreadElement,
    isStringLiteral,
  } = t;
  const { isPlatformNode, isPlatformSelectNode } = (0,
  _createInlinePlatformChecks.default)(t, options.requireName ?? "require");
  function isGlobal(binding) {
    return !binding;
  }
  const isFlowDeclared = (binding) => t.isDeclareVariable(binding.path);
  function isGlobalOrFlowDeclared(binding) {
    return !binding || isFlowDeclared(binding);
  }
  const isLeftHandSideOfAssignmentExpression = (node, parent) =>
    isAssignmentExpression(parent) && parent.left === node;
  const isProcessEnvNodeEnv = (node, scope) =>
    isIdentifier(node.property, nodeEnv) &&
    isMemberExpression(node.object) &&
    isIdentifier(node.object.property, env) &&
    isIdentifier(node.object.object, processId) &&
    isGlobal(scope.getBinding(processId.name));
  const isDev = (node, parent, scope) =>
    isIdentifier(node, dev) &&
    isGlobalOrFlowDeclared(scope.getBinding(dev.name));
  function findProperty(objectExpression, key, fallback) {
    let value = null;
    for (const p of objectExpression.properties) {
      if (!isObjectProperty(p) && !isObjectMethod(p)) {
        continue;
      }
      if (
        (isIdentifier(p.key) && p.key.name === key) ||
        (isStringLiteral(p.key) && p.key.value === key)
      ) {
        if (isObjectProperty(p)) {
          value = p.value;
          break;
        } else if (isObjectMethod(p)) {
          value = t.toExpression(p);
          break;
        }
      }
    }
    return value ?? fallback();
  }
  function hasStaticProperties(objectExpression) {
    return objectExpression.properties.every((p) => {
      if (p.computed === true || isSpreadElement(p)) {
        return false;
      }
      if (isObjectMethod(p) && p.kind !== "method") {
        return false;
      }
      return isIdentifier(p.key) || isStringLiteral(p.key);
    });
  }
  return {
    visitor: {
      ReferencedIdentifier(path, state) {
        if (!state.opts.dev && isDev(path.node, path.parent, path.scope)) {
          path.replaceWith(t.booleanLiteral(state.opts.dev));
        }
      },
      MemberExpression(path, state) {
        const node = path.node;
        const scope = path.scope;
        const opts = state.opts;
        if (!isLeftHandSideOfAssignmentExpression(node, path.parent)) {
          if (
            opts.inlinePlatform &&
            isPlatformNode(node, scope, !!opts.isWrapped)
          ) {
            path.replaceWith(t.stringLiteral(opts.platform));
          } else if (!opts.dev && isProcessEnvNodeEnv(node, scope)) {
            path.replaceWith(
              t.stringLiteral(opts.dev ? "development" : "production"),
            );
          }
        }
      },
      CallExpression(path, state) {
        const node = path.node;
        const scope = path.scope;
        const arg = node.arguments[0];
        const opts = state.opts;
        if (
          opts.inlinePlatform &&
          isPlatformSelectNode(node, scope, !!opts.isWrapped) &&
          isObjectExpression(arg)
        ) {
          if (hasStaticProperties(arg)) {
            const fallback = () =>
              findProperty(arg, "native", () =>
                findProperty(arg, "default", () => t.identifier("undefined")),
              );
            path.replaceWith(findProperty(arg, opts.platform, fallback));
          }
        }
      },
    },
  };
}
