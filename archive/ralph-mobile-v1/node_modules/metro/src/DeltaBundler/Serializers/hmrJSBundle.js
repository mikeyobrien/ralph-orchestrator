"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = hmrJSBundle;
var _js = require("./helpers/js");
var jscSafeUrl = _interopRequireWildcard(require("jsc-safe-url"));
var _metroTransformPlugins = require("metro-transform-plugins");
var _path = _interopRequireDefault(require("path"));
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
const debug = require("debug")("Metro:HMR");
function generateModules(sourceModules, graph, options) {
  const modules = [];
  for (const module of sourceModules) {
    if ((0, _js.isJsModule)(module)) {
      const getPathname = (extension) => {
        return _path.default
          .relative(
            options.serverRoot ?? options.projectRoot,
            _path.default.join(
              _path.default.dirname(module.path),
              _path.default.basename(
                module.path,
                _path.default.extname(module.path),
              ) +
                "." +
                extension,
            ),
          )
          .split(_path.default.sep)
          .map((segment) => encodeURIComponent(segment))
          .join("/");
      };
      const clientUrl = new URL(options.clientUrl);
      clientUrl.searchParams.delete("excludeSource");
      clientUrl.pathname = getPathname("map");
      const sourceMappingURL = clientUrl.toString();
      clientUrl.pathname = getPathname("bundle");
      const sourceURL = jscSafeUrl.toJscSafeUrl(clientUrl.toString());
      debug(
        "got sourceMappingURL: %s\nand sourceURL: %s\nfor module: %s",
        sourceMappingURL,
        sourceURL,
        module.path,
      );
      const code =
        prepareModule(module, graph, options) +
        `\n//# sourceMappingURL=${sourceMappingURL}\n` +
        `//# sourceURL=${sourceURL}\n`;
      modules.push({
        module: [options.createModuleId(module.path), code],
        sourceMappingURL,
        sourceURL,
      });
    }
  }
  return modules;
}
function prepareModule(module, graph, options) {
  const code = (0, _js.wrapModule)(module, {
    ...options,
    sourceUrl: options.clientUrl.toString(),
    dev: true,
  });
  const inverseDependencies = getInverseDependencies(module.path, graph);
  const inverseDependenciesById = Object.create(null);
  Object.keys(inverseDependencies).forEach((path) => {
    inverseDependenciesById[options.createModuleId(path)] = inverseDependencies[
      path
    ].map(options.createModuleId);
  });
  return (0, _metroTransformPlugins.addParamsToDefineCall)(
    code,
    inverseDependenciesById,
  );
}
function getInverseDependencies(path, graph, inverseDependencies = {}) {
  if (path in inverseDependencies) {
    return inverseDependencies;
  }
  const module = graph.dependencies.get(path);
  if (!module) {
    return inverseDependencies;
  }
  inverseDependencies[path] = [];
  for (const inverse of module.inverseDependencies) {
    inverseDependencies[path].push(inverse);
    getInverseDependencies(inverse, graph, inverseDependencies);
  }
  return inverseDependencies;
}
function hmrJSBundle(delta, graph, options) {
  return {
    added: generateModules(delta.added.values(), graph, options),
    modified: generateModules(delta.modified.values(), graph, options),
    deleted: [...delta.deleted].map((path) => options.createModuleId(path)),
  };
}
