"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.transform = exports.getCacheKey = exports.default = void 0;
var assetTransformer = _interopRequireWildcard(
  require("./utils/assetTransformer"),
);
var _getMinifier = _interopRequireDefault(require("./utils/getMinifier"));
var _core = require("@babel/core");
var _generator = _interopRequireDefault(require("@babel/generator"));
var babylon = _interopRequireWildcard(require("@babel/parser"));
var types = _interopRequireWildcard(require("@babel/types"));
var _metroCache = require("metro-cache");
var _metroCacheKey = require("metro-cache-key");
var _metroSourceMap = require("metro-source-map");
var _metroTransformPlugins = _interopRequireDefault(
  require("metro-transform-plugins"),
);
var _collectDependencies = _interopRequireDefault(
  require("metro/private/ModuleGraph/worker/collectDependencies"),
);
var _generateImportNames = _interopRequireDefault(
  require("metro/private/ModuleGraph/worker/generateImportNames"),
);
var _importLocationsPlugin = require("metro/private/ModuleGraph/worker/importLocationsPlugin");
var JsFileWrapping = _interopRequireWildcard(
  require("metro/private/ModuleGraph/worker/JsFileWrapping"),
);
var _nullthrows = _interopRequireDefault(require("nullthrows"));
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
const InternalInvalidRequireCallError =
  _collectDependencies.default.InvalidRequireCallError;
function getDynamicDepsBehavior(inPackages, filename) {
  switch (inPackages) {
    case "reject":
      return "reject";
    case "throwAtRuntime":
      const isPackage = /(?:^|[/\\])node_modules[/\\]/.test(filename);
      return isPackage ? inPackages : "reject";
    default:
      inPackages;
      throw new Error(
        `invalid value for dynamic deps behavior: \`${inPackages}\``,
      );
  }
}
const minifyCode = async (
  config,
  projectRoot,
  filename,
  code,
  source,
  map,
  reserved = [],
) => {
  const sourceMap = (0, _metroSourceMap.fromRawMappings)([
    {
      code,
      source,
      map,
      functionMap: null,
      path: filename,
      isIgnored: false,
    },
  ]).toMap(undefined, {});
  const minify = (0, _getMinifier.default)(config.minifierPath);
  try {
    const minified = await minify({
      code,
      map: sourceMap,
      filename,
      reserved,
      config: config.minifierConfig,
    });
    return {
      code: minified.code,
      map: minified.map
        ? (0, _metroSourceMap.toBabelSegments)(minified.map).map(
            _metroSourceMap.toSegmentTuple,
          )
        : [],
    };
  } catch (error) {
    if (error.constructor.name === "JS_Parse_Error") {
      throw new Error(
        `${error.message} in file ${filename} at ${error.line}:${error.col}`,
      );
    }
    throw error;
  }
};
const disabledDependencyTransformer = {
  transformSyncRequire: () => void 0,
  transformImportCall: () => void 0,
  transformImportMaybeSyncCall: () => void 0,
  transformPrefetch: () => void 0,
  transformIllegalDynamicRequire: () => void 0,
};
class InvalidRequireCallError extends Error {
  constructor(innerError, filename) {
    super(`${filename}:${innerError.message}`);
    this.innerError = innerError;
    this.filename = filename;
  }
}
async function transformJS(file, { config, options, projectRoot }) {
  let ast =
    file.ast ??
    babylon.parse(file.code, {
      sourceType: "unambiguous",
    });
  const { importDefault, importAll } = (0, _generateImportNames.default)(ast);
  const { directives } = ast.program;
  if (
    ast.program.sourceType === "module" &&
    directives != null &&
    directives.findIndex((d) => d.value.value === "use strict") === -1
  ) {
    directives.push(types.directive(types.directiveLiteral("use strict")));
  }
  const plugins = [];
  if (options.experimentalImportSupport === true) {
    plugins.push([
      _metroTransformPlugins.default.importExportPlugin,
      {
        importAll,
        importDefault,
        resolve: false,
      },
    ]);
  }
  if (options.inlineRequires) {
    plugins.push([
      _metroTransformPlugins.default.inlineRequiresPlugin,
      {
        ignoredRequires: options.nonInlinedRequires,
        inlineableCalls: [importDefault, importAll],
        memoizeCalls:
          options.customTransformOptions?.unstable_memoizeInlineRequires ??
          options.unstable_memoizeInlineRequires,
        nonMemoizedModules: options.unstable_nonMemoizedInlineRequires,
      },
    ]);
  }
  plugins.push([
    _metroTransformPlugins.default.inlinePlugin,
    {
      dev: options.dev,
      inlinePlatform: options.inlinePlatform,
      isWrapped: false,
      platform: options.platform,
    },
  ]);
  ast = (0, _nullthrows.default)(
    (0, _core.transformFromAstSync)(ast, "", {
      ast: true,
      babelrc: false,
      code: false,
      configFile: false,
      comments: true,
      filename: file.filename,
      plugins,
      sourceMaps: false,
      cloneInputAst: true,
    }).ast,
  );
  if (!options.dev) {
    ast = (0, _nullthrows.default)(
      (0, _core.transformFromAstSync)(ast, "", {
        ast: true,
        babelrc: false,
        code: false,
        configFile: false,
        comments: true,
        filename: file.filename,
        plugins: [_metroTransformPlugins.default.constantFoldingPlugin],
        sourceMaps: false,
        cloneInputAst: false,
      }).ast,
    );
  }
  let dependencyMapName = "";
  let dependencies;
  let wrappedAst;
  if (file.type === "js/script") {
    dependencies = [];
    wrappedAst = JsFileWrapping.wrapPolyfill(ast);
  } else {
    try {
      const importDeclarationLocs = file.unstable_importDeclarationLocs ?? null;
      const opts = {
        asyncRequireModulePath: config.asyncRequireModulePath,
        dependencyTransformer:
          config.unstable_disableModuleWrapping === true
            ? disabledDependencyTransformer
            : undefined,
        dynamicRequires: getDynamicDepsBehavior(
          config.dynamicDepsInPackages,
          file.filename,
        ),
        inlineableCalls: [importDefault, importAll],
        keepRequireNames: options.dev,
        allowOptionalDependencies: config.allowOptionalDependencies,
        dependencyMapName: config.unstable_dependencyMapReservedName,
        unstable_allowRequireContext: config.unstable_allowRequireContext,
        unstable_isESMImportAtSource:
          importDeclarationLocs != null
            ? (loc) =>
                importDeclarationLocs.has(
                  (0, _importLocationsPlugin.locToKey)(loc),
                )
            : null,
      };
      ({ ast, dependencies, dependencyMapName } = (0,
      _collectDependencies.default)(ast, opts));
    } catch (error) {
      if (error instanceof InternalInvalidRequireCallError) {
        throw new InvalidRequireCallError(error, file.filename);
      }
      throw error;
    }
    if (config.unstable_disableModuleWrapping === true) {
      wrappedAst = ast;
    } else {
      ({ ast: wrappedAst } = JsFileWrapping.wrapModule(
        ast,
        importDefault,
        importAll,
        dependencyMapName,
        config.globalPrefix,
        config.unstable_renameRequire === false,
        {
          unstable_useStaticHermesModuleFactory: Boolean(
            options.customTransformOptions
              ?.unstable_staticHermesOptimizedRequire,
          ),
        },
      ));
    }
  }
  const minify =
    options.minify &&
    options.unstable_transformProfile !== "hermes-canary" &&
    options.unstable_transformProfile !== "hermes-stable";
  const reserved = [];
  if (config.unstable_dependencyMapReservedName != null) {
    reserved.push(config.unstable_dependencyMapReservedName);
  }
  if (
    minify &&
    file.inputFileSize <= config.optimizationSizeLimit &&
    !config.unstable_disableNormalizePseudoGlobals
  ) {
    reserved.push(
      ..._metroTransformPlugins.default.normalizePseudoGlobals(wrappedAst, {
        reservedNames: reserved,
      }),
    );
  }
  const result = (0, _generator.default)(
    wrappedAst,
    {
      comments: true,
      compact: config.unstable_compactOutput,
      filename: file.filename,
      retainLines: false,
      sourceFileName: file.filename,
      sourceMaps: true,
    },
    file.code,
  );
  let map = result.rawMappings
    ? result.rawMappings.map(_metroSourceMap.toSegmentTuple)
    : [];
  let code = result.code;
  if (minify) {
    ({ map, code } = await minifyCode(
      config,
      projectRoot,
      file.filename,
      result.code,
      file.code,
      map,
      reserved,
    ));
  }
  let lineCount;
  ({ lineCount, map } = countLinesAndTerminateMap(code, map));
  const output = [
    {
      data: {
        code,
        lineCount,
        map,
        functionMap: file.functionMap,
      },
      type: file.type,
    },
  ];
  return {
    dependencies,
    output,
  };
}
async function transformAsset(file, context) {
  const { assetRegistryPath, assetPlugins } = context.config;
  const result = await assetTransformer.transform(
    getBabelTransformArgs(file, context),
    assetRegistryPath,
    assetPlugins,
  );
  const jsFile = {
    ...file,
    type: "js/module/asset",
    ast: result.ast,
    functionMap: null,
  };
  return transformJS(jsFile, context);
}
async function transformJSWithBabel(file, context) {
  const { babelTransformerPath } = context.config;
  const transformer = require(babelTransformerPath);
  const transformResult = await transformer.transform(
    getBabelTransformArgs(file, context, [
      _metroSourceMap.functionMapBabelPlugin,
      _importLocationsPlugin.importLocationsPlugin,
    ]),
  );
  const jsFile = {
    ...file,
    ast: transformResult.ast,
    functionMap:
      transformResult.metadata?.metro?.functionMap ??
      transformResult.functionMap ??
      null,
    unstable_importDeclarationLocs:
      transformResult.metadata?.metro?.unstable_importDeclarationLocs,
  };
  return await transformJS(jsFile, context);
}
async function transformJSON(file, { options, config, projectRoot }) {
  let code =
    config.unstable_disableModuleWrapping === true
      ? JsFileWrapping.jsonToCommonJS(file.code)
      : JsFileWrapping.wrapJson(
          file.code,
          config.globalPrefix,
          Boolean(
            options.customTransformOptions
              ?.unstable_staticHermesOptimizedRequire,
          ),
        );
  let map = [];
  const minify =
    options.minify &&
    options.unstable_transformProfile !== "hermes-canary" &&
    options.unstable_transformProfile !== "hermes-stable";
  if (minify) {
    ({ map, code } = await minifyCode(
      config,
      projectRoot,
      file.filename,
      code,
      file.code,
      map,
    ));
  }
  let jsType;
  if (file.type === "asset") {
    jsType = "js/module/asset";
  } else if (file.type === "script") {
    jsType = "js/script";
  } else {
    jsType = "js/module";
  }
  let lineCount;
  ({ lineCount, map } = countLinesAndTerminateMap(code, map));
  const output = [
    {
      data: {
        code,
        lineCount,
        map,
        functionMap: null,
      },
      type: jsType,
    },
  ];
  return {
    dependencies: [],
    output,
  };
}
function getBabelTransformArgs(
  file,
  { options, config, projectRoot },
  plugins = [],
) {
  const { inlineRequires: _, ...babelTransformerOptions } = options;
  return {
    filename: file.filename,
    options: {
      ...babelTransformerOptions,
      enableBabelRCLookup: config.enableBabelRCLookup,
      enableBabelRuntime: config.enableBabelRuntime,
      globalPrefix: config.globalPrefix,
      hermesParser: config.hermesParser,
      projectRoot,
      publicPath: config.publicPath,
    },
    plugins,
    src: file.code,
  };
}
const transform = async (config, projectRoot, filename, data, options) => {
  const context = {
    config,
    projectRoot,
    options,
  };
  const sourceCode = data.toString("utf8");
  const reservedStrings = [];
  if (
    options.customTransformOptions?.unstable_staticHermesOptimizedRequire ==
    true
  ) {
    reservedStrings.push("_$$_METRO_MODULE_ID");
  }
  if (config.unstable_dependencyMapReservedName != null) {
    reservedStrings.push(config.unstable_dependencyMapReservedName);
  }
  for (const reservedString of reservedStrings) {
    const position = sourceCode.indexOf(reservedString);
    if (position > -1) {
      throw new SyntaxError(
        "Source code contains the reserved string `" +
          reservedString +
          "` at character offset " +
          position,
      );
    }
  }
  if (filename.endsWith(".json")) {
    const jsonFile = {
      filename,
      inputFileSize: data.length,
      code: sourceCode,
      type: options.type,
    };
    return await transformJSON(jsonFile, context);
  }
  if (options.type === "asset") {
    const file = {
      filename,
      inputFileSize: data.length,
      code: sourceCode,
      type: options.type,
    };
    return await transformAsset(file, context);
  }
  const file = {
    filename,
    inputFileSize: data.length,
    code: sourceCode,
    type: options.type === "script" ? "js/script" : "js/module",
    functionMap: null,
  };
  return await transformJSWithBabel(file, context);
};
exports.transform = transform;
const getCacheKey = (config) => {
  const { babelTransformerPath, minifierPath, ...remainingConfig } = config;
  const filesKey = (0, _metroCacheKey.getCacheKey)([
    __filename,
    require.resolve(babelTransformerPath),
    require.resolve(minifierPath),
    require.resolve("./utils/getMinifier"),
    require.resolve("./utils/assetTransformer"),
    require.resolve("metro/private/ModuleGraph/worker/generateImportNames"),
    require.resolve("metro/private/ModuleGraph/worker/JsFileWrapping"),
    ..._metroTransformPlugins.default.getTransformPluginCacheKeyFiles(),
  ]);
  const babelTransformer = require(babelTransformerPath);
  return [
    filesKey,
    (0, _metroCache.stableHash)(remainingConfig).toString("hex"),
    babelTransformer.getCacheKey ? babelTransformer.getCacheKey() : "",
  ].join("$");
};
exports.getCacheKey = getCacheKey;
function countLinesAndTerminateMap(code, map) {
  const NEWLINE = /\r\n?|\n|\u2028|\u2029/g;
  let lineCount = 1;
  let lastLineStart = 0;
  for (const match of code.matchAll(NEWLINE)) {
    lineCount++;
    lastLineStart = match.index + match[0].length;
  }
  const lastLineLength = code.length - lastLineStart;
  const lastLineIndex1Based = lineCount;
  const lastLineNextColumn0Based = lastLineLength;
  const lastMapping = map[map.length - 1];
  const terminatingMapping = [lastLineIndex1Based, lastLineNextColumn0Based];
  if (
    !lastMapping ||
    lastMapping[0] !== terminatingMapping[0] ||
    lastMapping[1] !== terminatingMapping[1]
  ) {
    return {
      lineCount,
      map: map.concat([terminatingMapping]),
    };
  }
  return {
    lineCount,
    map: [...map],
  };
}
var _default = (exports.default = {
  getCacheKey,
  transform,
});
