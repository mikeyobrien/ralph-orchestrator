"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.createRamBundleGroups = createRamBundleGroups;
exports.generateAssetCodeFileAst = generateAssetCodeFileAst;
var babylon = _interopRequireWildcard(require("@babel/parser"));
var _template = _interopRequireDefault(require("@babel/template"));
var babelTypes = _interopRequireWildcard(require("@babel/types"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
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
const assetPropertyBlockList = new Set(["files", "fileSystemLocation", "path"]);
function generateAssetCodeFileAst(assetRegistryPath, assetDescriptor) {
  const properDescriptor = filterObject(
    assetDescriptor,
    assetPropertyBlockList,
  );
  const descriptorAst = babylon.parseExpression(
    JSON.stringify(properDescriptor),
  );
  const t = babelTypes;
  const buildRequire = _template.default.statement(`
    module.exports = require(ASSET_REGISTRY_PATH).registerAsset(DESCRIPTOR_AST)
  `);
  return t.file(
    t.program([
      buildRequire({
        ASSET_REGISTRY_PATH: t.stringLiteral(assetRegistryPath),
        DESCRIPTOR_AST: descriptorAst,
      }),
    ]),
  );
}
function filterObject(object, blockList) {
  const copied = {
    ...object,
  };
  for (const key of blockList) {
    delete copied[key];
  }
  return copied;
}
function createRamBundleGroups(ramGroups, groupableModules, subtree) {
  const byPath = new Map();
  const byId = new Map();
  groupableModules.forEach((m) => {
    byPath.set(m.sourcePath, m);
    byId.set(m.id, m.sourcePath);
  });
  const result = new Map(
    ramGroups.map((modulePath) => {
      const root = byPath.get(modulePath);
      if (root == null) {
        throw Error(`Group root ${modulePath} is not part of the bundle`);
      }
      return [root.id, new Set(subtree(root, byPath))];
    }),
  );
  if (ramGroups.length > 1) {
    const all = new ArrayMap();
    for (const [parent, children] of result) {
      for (const module of children) {
        all.get(module).push(parent);
      }
    }
    const doubles = filter(all, ([, parents]) => parents.length > 1);
    for (const [moduleId, parents] of doubles) {
      const parentNames = parents.map(byId.get, byId);
      const lastName = parentNames.pop();
      throw new Error(
        `Module ${byId.get(moduleId) || moduleId} belongs to groups ${parentNames.join(", ")}, and ${String(lastName)}. Ensure that each module is only part of one group.`,
      );
    }
  }
  return result;
}
function* filter(iterator, predicate) {
  for (const value of iterator) {
    if (predicate(value)) {
      yield value;
    }
  }
}
class ArrayMap extends Map {
  get(key) {
    let array = super.get(key);
    if (!array) {
      array = [];
      this.set(key, array);
    }
    return array;
  }
}
