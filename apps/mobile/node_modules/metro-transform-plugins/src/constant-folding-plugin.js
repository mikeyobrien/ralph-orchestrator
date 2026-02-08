"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = constantFoldingPlugin;
function constantFoldingPlugin(context) {
  const t = context.types;
  const { isLiteral, isVariableDeclarator, isUnaryExpression } = t;
  const traverse = context.traverse;
  const evaluate = function (path) {
    const state = {
      safe: true,
    };
    const unsafe = (path, state) => {
      state.safe = false;
    };
    if (isUnaryExpression(path.node) && path.node.operator === "void") {
      if (isLiteral(path.node.argument)) {
        return {
          confident: true,
          value: undefined,
        };
      }
      return {
        confident: false,
        value: null,
      };
    }
    path.traverse(
      {
        AssignmentExpression: unsafe,
        CallExpression: unsafe,
        OptionalCallExpression: unsafe,
      },
      state,
    );
    try {
      if (!state.safe) {
        return {
          confident: false,
          value: null,
        };
      }
      const evaluated = path.evaluate();
      return {
        confident: evaluated.confident,
        value: evaluated.value,
      };
    } catch {
      return {
        confident: false,
        value: null,
      };
    }
  };
  const FunctionDeclaration = {
    exit(path, state) {
      const binding =
        path.node.id != null && path.scope.parent.getBinding(path.node.id.name);
      if (binding && !binding.referenced) {
        state.stripped = true;
        path.remove();
      }
    },
  };
  const FunctionExpression = {
    exit(path, state) {
      const parentPath = path.parentPath;
      const parentNode = parentPath?.node;
      if (isVariableDeclarator(parentNode) && parentNode.id.name != null) {
        const binding = parentPath?.scope.getBinding(parentNode.id.name);
        if (binding && !binding.referenced) {
          state.stripped = true;
          parentPath?.remove();
        }
      }
    },
  };
  const Conditional = {
    exit(path, state) {
      const node = path.node;
      const result = evaluate(path.get("test"));
      if (result.confident) {
        state.stripped = true;
        if (result.value || node.alternate) {
          path.replaceWith(result.value ? node.consequent : node.alternate);
        } else if (!result.value) {
          path.remove();
        }
      }
    },
  };
  const Expression = {
    exit(path) {
      const result = evaluate(path);
      if (result.confident) {
        path.replaceWith(t.valueToNode(result.value));
        path.skip();
      }
    },
  };
  const LogicalExpression = {
    exit(path) {
      const node = path.node;
      const result = evaluate(path.get("left"));
      if (result.confident) {
        const value = result.value;
        switch (node.operator) {
          case "||":
            path.replaceWith(value ? node.left : node.right);
            break;
          case "&&":
            path.replaceWith(value ? node.right : node.left);
            break;
          case "??":
            path.replaceWith(value == null ? node.right : node.left);
            break;
        }
      }
    },
  };
  const Program = {
    enter(path, state) {
      state.stripped = false;
    },
    exit(path, state) {
      path.traverse(
        {
          ArrowFunctionExpression: FunctionExpression,
          ConditionalExpression: Conditional,
          FunctionDeclaration,
          FunctionExpression,
          IfStatement: Conditional,
        },
        state,
      );
      if (state.stripped) {
        traverse.cache.clearScope();
        path.scope.crawl();
        Program.enter(path, state);
        path.traverse(visitor, {
          stripped: false,
        });
        Program.exit(path, state);
      }
    },
  };
  const visitor = {
    BinaryExpression: Expression,
    LogicalExpression,
    Program: {
      ...Program,
    },
    UnaryExpression: Expression,
  };
  return {
    visitor,
  };
}
