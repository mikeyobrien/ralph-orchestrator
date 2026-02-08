"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.functionMapBabelPlugin = functionMapBabelPlugin;
exports.generateFunctionMap = generateFunctionMap;
exports.generateFunctionMappingsArray = generateFunctionMappingsArray;
var _B64Builder = _interopRequireDefault(require("./B64Builder"));
var _traverseForGenerateFunctionMap = _interopRequireDefault(
  require("@babel/traverse--for-generate-function-map"),
);
var _types = _interopRequireWildcard(require("@babel/types"));
var t = _types;
var _invariant = _interopRequireDefault(require("invariant"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
var _path = _interopRequireDefault(require("path"));
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
  return ((n.default = e), t && t.set(e, n), n);
}
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function generateFunctionMap(ast, context) {
  const encoder = new MappingEncoder();
  forEachMapping(ast, context, (mapping) => encoder.push(mapping));
  return encoder.getResult();
}
function generateFunctionMappingsArray(ast, context) {
  const mappings = [];
  forEachMapping(ast, context, (mapping) => {
    mappings.push(mapping);
  });
  return mappings;
}
function functionMapBabelPlugin() {
  return {
    visitor: {},
    pre: ({ path, metadata, opts }) => {
      const { filename } = (0, _nullthrows.default)(opts);
      const encoder = new MappingEncoder();
      const visitor = getFunctionMapVisitor(
        {
          filename,
        },
        (mapping) => encoder.push(mapping),
      );
      (0, _invariant.default)(
        path && t.isProgram(path.node),
        "path missing or not a program node",
      );
      const programPath = path;
      visitor.enter(programPath);
      programPath.traverse({
        Function: visitor,
        Class: visitor,
      });
      visitor.exit(programPath);
      const metroMetadata = metadata;
      const functionMap = encoder.getResult();
      if (!metroMetadata.metro) {
        metroMetadata.metro = {
          functionMap,
        };
      } else {
        metroMetadata.metro.functionMap = functionMap;
      }
    },
  };
}
function getFunctionMapVisitor(context, pushMapping) {
  const nameStack = [];
  let tailPos = {
    line: 1,
    column: 0,
  };
  let tailName = null;
  function advanceToPos(pos) {
    if (tailPos && positionGreater(pos, tailPos)) {
      const name = nameStack[0].name;
      if (name !== tailName) {
        pushMapping({
          name,
          start: {
            line: tailPos.line,
            column: tailPos.column,
          },
        });
        tailName = name;
      }
    }
    tailPos = pos;
  }
  function pushFrame(name, loc) {
    advanceToPos(loc.start);
    nameStack.unshift({
      name,
      loc,
    });
  }
  function popFrame() {
    const top = nameStack[0];
    if (top) {
      const { loc } = top;
      advanceToPos(loc.end);
      nameStack.shift();
    }
  }
  if (!context) {
    context = {};
  }
  const basename = context.filename
    ? _path.default.basename(context.filename).replace(/\..+$/, "")
    : null;
  return {
    enter(path) {
      let name = getNameForPath(path);
      if (basename) {
        name = removeNamePrefix(name, basename);
      }
      pushFrame(name, (0, _nullthrows.default)(path.node.loc));
    },
    exit(path) {
      popFrame();
    },
  };
}
function forEachMapping(ast, context, pushMapping) {
  const visitor = getFunctionMapVisitor(context, pushMapping);
  (0, _traverseForGenerateFunctionMap.default)(ast, {
    noScope: true,
    Function: visitor,
    Program: visitor,
    Class: visitor,
  });
}
const ANONYMOUS_NAME = "<anonymous>";
function getNameForPath(path) {
  const { node, parent, parentPath } = path;
  if ((0, _types.isProgram)(node)) {
    return "<global>";
  }
  let { id } = path;
  if (node.id) {
    return node.id.name;
  }
  let propertyPath;
  let kind;
  if ((0, _types.isObjectMethod)(node) || (0, _types.isClassMethod)(node)) {
    id = node.key;
    if (node.kind !== "method" && node.kind !== "constructor") {
      kind = node.kind;
    }
    propertyPath = path;
  } else if (
    (0, _types.isObjectProperty)(parent) ||
    (0, _types.isClassProperty)(parent)
  ) {
    id = parent.key;
    propertyPath = parentPath;
  } else if ((0, _types.isVariableDeclarator)(parent)) {
    id = parent.id;
  } else if ((0, _types.isAssignmentExpression)(parent)) {
    id = parent.left;
  } else if ((0, _types.isJSXExpressionContainer)(parent)) {
    const grandParentNode = parentPath?.parentPath?.node;
    if ((0, _types.isJSXElement)(grandParentNode)) {
      const openingElement = grandParentNode.openingElement;
      id = t.jsxMemberExpression(
        t.jsxMemberExpression(openingElement.name, t.jsxIdentifier("props")),
        t.jsxIdentifier("children"),
      );
    } else if ((0, _types.isJSXAttribute)(grandParentNode)) {
      const openingElement = parentPath?.parentPath?.parentPath?.node;
      const prop = grandParentNode;
      id = t.jsxMemberExpression(
        t.jsxMemberExpression(openingElement.name, t.jsxIdentifier("props")),
        prop.name,
      );
    }
  }
  let name = getNameFromId(id);
  if (name == null) {
    if (isAnyCallExpression(parent)) {
      const argIndex = parent.arguments.indexOf(node);
      if (argIndex !== -1) {
        const calleeName = getNameFromId(parent.callee);
        if (argIndex === 0 && calleeName === "Object.freeze") {
          return getNameForPath((0, _nullthrows.default)(parentPath));
        }
        if (
          argIndex === 0 &&
          (calleeName === "useCallback" || calleeName === "React.useCallback")
        ) {
          return getNameForPath((0, _nullthrows.default)(parentPath));
        }
        if (calleeName) {
          return `${calleeName}$argument_${argIndex}`;
        }
      }
    }
    if (
      (0, _types.isTypeCastExpression)(parent) &&
      parent.expression === node
    ) {
      return getNameForPath((0, _nullthrows.default)(parentPath));
    }
    if ((0, _types.isExportDefaultDeclaration)(parent)) {
      return "default";
    }
    return ANONYMOUS_NAME;
  }
  if (kind != null) {
    name = kind + "__" + name;
  }
  if (propertyPath) {
    if ((0, _types.isClassBody)(propertyPath.parent)) {
      const className = getNameForPath(propertyPath.parentPath.parentPath);
      if (className !== ANONYMOUS_NAME) {
        const separator = propertyPath.node.static ? "." : "#";
        name = className + separator + name;
      }
    } else if ((0, _types.isObjectExpression)(propertyPath.parent)) {
      const objectName = getNameForPath(
        (0, _nullthrows.default)(propertyPath.parentPath),
      );
      if (objectName !== ANONYMOUS_NAME) {
        name = objectName + "." + name;
      }
    }
  }
  return name;
}
function isAnyCallExpression(node) {
  return (
    node.type === "CallExpression" ||
    node.type === "NewExpression" ||
    node.type === "OptionalCallExpression"
  );
}
function isAnyMemberExpression(node) {
  return (
    node.type === "MemberExpression" ||
    node.type === "JSXMemberExpression" ||
    node.type === "OptionalMemberExpression"
  );
}
function isAnyIdentifier(node) {
  return (0, _types.isIdentifier)(node) || (0, _types.isJSXIdentifier)(node);
}
function getNameFromId(id) {
  const parts = getNamePartsFromId(id);
  if (!parts.length) {
    return null;
  }
  if (parts.length > 5) {
    return (
      parts[0] +
      "." +
      parts[1] +
      "..." +
      parts[parts.length - 2] +
      "." +
      parts[parts.length - 1]
    );
  }
  return parts.join(".");
}
function getNamePartsFromId(id) {
  if (!id) {
    return [];
  }
  if (isAnyCallExpression(id)) {
    return getNamePartsFromId(id.callee);
  }
  if ((0, _types.isTypeCastExpression)(id)) {
    return getNamePartsFromId(id.expression);
  }
  let name;
  if (isAnyIdentifier(id)) {
    name = id.name;
  } else if ((0, _types.isNullLiteral)(id)) {
    name = "null";
  } else if ((0, _types.isRegExpLiteral)(id)) {
    name = `_${id.pattern}_${id.flags ?? ""}`;
  } else if ((0, _types.isTemplateLiteral)(id)) {
    name = id.quasis.map((quasi) => quasi.value.raw).join("");
  } else if ((0, _types.isLiteral)(id) && id.value != null) {
    name = String(id.value);
  }
  if (name != null) {
    return [t.toBindingIdentifierName(name)];
  }
  if ((0, _types.isImport)(id)) {
    name = "import";
  }
  if (name != null) {
    return [name];
  }
  if (isAnyMemberExpression(id)) {
    if (
      isAnyIdentifier(id.object) &&
      id.object.name === "Symbol" &&
      (isAnyIdentifier(id.property) || (0, _types.isLiteral)(id.property))
    ) {
      const propertyName = getNameFromId(id.property);
      if (propertyName) {
        name = "@@" + propertyName;
      }
    } else {
      const propertyName = getNamePartsFromId(id.property);
      if (propertyName.length) {
        const objectName = getNamePartsFromId(id.object);
        if (objectName.length) {
          return [...objectName, ...propertyName];
        } else {
          return propertyName;
        }
      }
    }
  }
  return name ? [name] : [];
}
const DELIMITER_START_RE = /^[^A-Za-z0-9_$@]+/;
function removeNamePrefix(name, namePrefix) {
  if (!namePrefix.length || !name.startsWith(namePrefix)) {
    return name;
  }
  const shortenedName = name.substr(namePrefix.length);
  const [delimiterMatch] = shortenedName.match(DELIMITER_START_RE) || [];
  if (delimiterMatch) {
    return shortenedName.substr(delimiterMatch.length) || name;
  }
  return name;
}
class MappingEncoder {
  constructor() {
    this._namesMap = new Map();
    this._names = [];
    this._line = new RelativeValue(1);
    this._column = new RelativeValue(0);
    this._nameIndex = new RelativeValue(0);
    this._mappings = new _B64Builder.default();
  }
  getResult() {
    return {
      names: this._names,
      mappings: this._mappings.toString(),
    };
  }
  push({ name, start }) {
    let nameIndex = this._namesMap.get(name);
    if (typeof nameIndex !== "number") {
      nameIndex = this._names.length;
      this._names[nameIndex] = name;
      this._namesMap.set(name, nameIndex);
    }
    const lineDelta = this._line.next(start.line);
    const firstOfLine = this._mappings.pos === 0 || lineDelta > 0;
    if (lineDelta > 0) {
      this._mappings.markLines(1);
      this._column.reset(0);
    }
    this._mappings.startSegment(this._column.next(start.column));
    this._mappings.append(this._nameIndex.next(nameIndex));
    if (firstOfLine) {
      this._mappings.append(lineDelta);
    }
  }
}
class RelativeValue {
  constructor(value = 0) {
    this.reset(value);
  }
  next(absoluteValue) {
    const delta = absoluteValue - this._value;
    this._value = absoluteValue;
    return delta;
  }
  reset(value = 0) {
    this._value = value;
  }
}
function positionGreater(x, y) {
  return x.line > y.line || (x.line === y.line && x.column > y.column);
}
