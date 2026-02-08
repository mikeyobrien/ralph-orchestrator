"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _defaults = require("./defaults");
var _exclusionList = _interopRequireDefault(require("./exclusionList"));
var _getMaxWorkers = _interopRequireDefault(require("./getMaxWorkers"));
var _metroCache = require("metro-cache");
var _metroCore = require("metro-core");
var _TerminalReporter = _interopRequireDefault(
  require("metro/private/lib/TerminalReporter"),
);
var os = _interopRequireWildcard(require("os"));
var path = _interopRequireWildcard(require("path"));
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
const getDefaultValues = (projectRoot) => ({
  resolver: {
    assetExts: _defaults.assetExts,
    assetResolutions: _defaults.assetResolutions,
    platforms: _defaults.platforms,
    sourceExts: _defaults.sourceExts,
    blockList: (0, _exclusionList.default)(),
    dependencyExtractor: undefined,
    disableHierarchicalLookup: false,
    emptyModulePath: require.resolve(
      "metro-runtime/src/modules/empty-module.js",
    ),
    enableGlobalPackages: false,
    extraNodeModules: {},
    hasteImplModulePath: undefined,
    nodeModulesPaths: [],
    resolveRequest: null,
    resolverMainFields: ["browser", "main"],
    unstable_conditionNames: [],
    unstable_conditionsByPlatform: {
      web: ["browser"],
    },
    unstable_enablePackageExports: true,
    useWatchman: true,
    requireCycleIgnorePatterns: [/(^|\/|\\)node_modules($|\/|\\)/],
  },
  serializer: {
    polyfillModuleNames: [],
    getRunModuleStatement: (moduleId, globalPrefix) =>
      `__r(${JSON.stringify(moduleId)});`,
    getPolyfills: () => [],
    getModulesRunBeforeMainModule: () => [],
    processModuleFilter: (module) => true,
    createModuleIdFactory: _defaults.defaultCreateModuleIdFactory,
    experimentalSerializerHook: () => {},
    customSerializer: null,
    isThirdPartyModule: (module) =>
      /(?:^|[/\\])node_modules[/\\]/.test(module.path),
  },
  server: {
    enhanceMiddleware: (middleware, _) => middleware,
    forwardClientLogs: true,
    port: 8081,
    rewriteRequestUrl: (url) => url,
    unstable_serverRoot: null,
    useGlobalHotkey: true,
    verifyConnections: false,
  },
  symbolicator: {
    customizeFrame: () => {},
    customizeStack: async (stack, _) => stack,
  },
  transformer: {
    assetPlugins: [],
    asyncRequireModulePath: "metro-runtime/src/modules/asyncRequire",
    assetRegistryPath: "missing-asset-registry-path",
    babelTransformerPath: "metro-babel-transformer",
    dynamicDepsInPackages: "throwAtRuntime",
    enableBabelRCLookup: true,
    enableBabelRuntime: true,
    getTransformOptions: async () => ({
      transform: {
        experimentalImportSupport: false,
        inlineRequires: false,
      },
      preloadedModules: false,
      ramGroups: [],
    }),
    globalPrefix: "",
    hermesParser: false,
    minifierConfig: {
      mangle: {
        toplevel: false,
      },
      output: {
        ascii_only: true,
        quote_style: 3,
        wrap_iife: true,
      },
      sourceMap: {
        includeSources: false,
      },
      toplevel: false,
      compress: {
        reduce_funcs: false,
      },
    },
    minifierPath: _defaults.DEFAULT_METRO_MINIFIER_PATH,
    optimizationSizeLimit: 150 * 1024,
    transformVariants: {
      default: {},
    },
    publicPath: "/assets",
    allowOptionalDependencies: false,
    unstable_allowRequireContext: false,
    unstable_dependencyMapReservedName: null,
    unstable_disableModuleWrapping: false,
    unstable_disableNormalizePseudoGlobals: false,
    unstable_renameRequire: true,
    unstable_compactOutput: false,
    unstable_memoizeInlineRequires: false,
    unstable_workerThreads: false,
  },
  watcher: {
    additionalExts: _defaults.additionalExts,
    healthCheck: {
      enabled: false,
      filePrefix: ".metro-health-check",
      interval: 30000,
      timeout: 5000,
    },
    unstable_lazySha1: true,
    unstable_workerThreads: false,
    unstable_autoSaveCache: {
      enabled: true,
      debounceMs: 5000,
    },
    watchman: {
      deferStates: ["hg.update"],
    },
  },
  cacheStores: [
    new _metroCache.FileStore({
      root: path.join(os.tmpdir(), "metro-cache"),
    }),
  ],
  cacheVersion: "1.0",
  projectRoot: projectRoot || path.resolve(__dirname, "../../.."),
  stickyWorkers: true,
  watchFolders: [],
  transformerPath: "metro-transform-worker",
  maxWorkers: (0, _getMaxWorkers.default)(),
  resetCache: false,
  reporter: new _TerminalReporter.default(
    new _metroCore.Terminal(process.stdout),
  ),
  unstable_perfLoggerFactory: _defaults.noopPerfLoggerFactory,
});
async function getDefaultConfig(rootPath) {
  return getDefaultValues(rootPath);
}
getDefaultConfig.getDefaultValues = getDefaultValues;
var _default = (exports.default = getDefaultConfig);
