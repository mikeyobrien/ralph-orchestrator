"use strict";

const getMinifier = require("./utils/getMinifier");
const { transformFromAstSync } = require("@babel/core");
const generate = require("@babel/generator").default;
const babylon = require("@babel/parser");
const types = require("@babel/types");
const { stableHash } = require("metro-cache");
const getCacheKey = require("metro-cache-key");
const {
  fromRawMappings,
  functionMapBabelPlugin,
  toBabelSegments,
  toSegmentTuple,
} = require("metro-source-map");
const metroTransformPlugins = require("metro-transform-plugins");
const collectDependencies = require("metro/src/ModuleGraph/worker/collectDependencies");
const {
  InvalidRequireCallError: InternalInvalidRequireCallError,
} = require("metro/src/ModuleGraph/worker/collectDependencies");
const generateImportNames = require("metro/src/ModuleGraph/worker/generateImportNames");
const {
  importLocationsPlugin,
  locToKey,
} = require("metro/src/ModuleGraph/worker/importLocationsPlugin");
const JsFileWrapping = require("metro/src/ModuleGraph/worker/JsFileWrapping");
const nullthrows = require("nullthrows");
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
        `invalid value for dynamic deps behavior: \`${inPackages}\``
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
  reserved = []
) => {
  const sourceMap = fromRawMappings([
    {
      code,
      source,
      map,
      functionMap: null,
      path: filename,
      isIgnored: false,
    },
  ]).toMap(undefined, {});
  const minify = getMinifier(config.minifierPath);
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
        ? toBabelSegments(minified.map).map(toSegmentTuple)
        : [],
    };
  } catch (error) {
    if (error.constructor.name === "JS_Parse_Error") {
      throw new Error(
        `${error.message} in file ${filename} at ${error.line}:${error.col}`
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
  const { importDefault, importAll } = generateImportNames(ast);
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
      metroTransformPlugins.importExportPlugin,
      {
        importAll,
        importDefault,
        resolve: false,
      },
    ]);
  }
  if (options.inlineRequires) {
    plugins.push([
      metroTransformPlugins.inlineRequiresPlugin,
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
    metroTransformPlugins.inlinePlugin,
    {
      dev: options.dev,
      inlinePlatform: options.inlinePlatform,
      isWrapped: false,
      platform: options.platform,
    },
  ]);
  ast = nullthrows(
    transformFromAstSync(ast, "", {
      ast: true,
      babelrc: false,
      code: false,
      configFile: false,
      comments: true,
      filename: file.filename,
      plugins,
      sourceMaps: false,
      cloneInputAst: true,
    }).ast
  );
  if (!options.dev) {
    ast = nullthrows(
      transformFromAstSync(ast, "", {
        ast: true,
        babelrc: false,
        code: false,
        configFile: false,
        comments: true,
        filename: file.filename,
        plugins: [metroTransformPlugins.constantFoldingPlugin],
        sourceMaps: false,
        cloneInputAst: false,
      }).ast
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
          file.filename
        ),
        inlineableCalls: [importDefault, importAll],
        keepRequireNames: options.dev,
        allowOptionalDependencies: config.allowOptionalDependencies,
        dependencyMapName: config.unstable_dependencyMapReservedName,
        unstable_allowRequireContext: config.unstable_allowRequireContext,
        unstable_isESMImportAtSource:
          importDeclarationLocs != null
            ? (loc) => importDeclarationLocs.has(locToKey(loc))
            : null,
      };
      ({ ast, dependencies, dependencyMapName } = collectDependencies(
        ast,
        opts
      ));
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
        config.unstable_renameRequire === false
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
      ...metroTransformPlugins.normalizePseudoGlobals(wrappedAst, {
        reservedNames: reserved,
      })
    );
  }
  const result = generate(
    wrappedAst,
    {
      comments: true,
      compact: config.unstable_compactOutput,
      filename: file.filename,
      retainLines: false,
      sourceFileName: file.filename,
      sourceMaps: true,
    },
    file.code
  );
  let map = result.rawMappings ? result.rawMappings.map(toSegmentTuple) : [];
  let code = result.code;
  if (minify) {
    ({ map, code } = await minifyCode(
      config,
      projectRoot,
      file.filename,
      result.code,
      file.code,
      map,
      reserved
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
  const assetTransformer = require("./utils/assetTransformer");
  const { assetRegistryPath, assetPlugins } = context.config;
  const result = await assetTransformer.transform(
    getBabelTransformArgs(file, context),
    assetRegistryPath,
    assetPlugins
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
      functionMapBabelPlugin,
      importLocationsPlugin,
    ])
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
      : JsFileWrapping.wrapJson(file.code, config.globalPrefix);
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
      map
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
  plugins = []
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
module.exports = {
  transform: async (config, projectRoot, filename, data, options) => {
    const context = {
      config,
      projectRoot,
      options,
    };
    const sourceCode = data.toString("utf8");
    const { unstable_dependencyMapReservedName } = config;
    if (unstable_dependencyMapReservedName != null) {
      const position = sourceCode.indexOf(unstable_dependencyMapReservedName);
      if (position > -1) {
        throw new SyntaxError(
          "Source code contains the reserved string `" +
            unstable_dependencyMapReservedName +
            "` at character offset " +
            position
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
  },
  getCacheKey: (config) => {
    const { babelTransformerPath, minifierPath, ...remainingConfig } = config;
    const filesKey = getCacheKey([
      __filename,
      require.resolve(babelTransformerPath),
      require.resolve(minifierPath),
      require.resolve("./utils/getMinifier"),
      require.resolve("./utils/assetTransformer"),
      require.resolve("metro/src/ModuleGraph/worker/generateImportNames"),
      require.resolve("metro/src/ModuleGraph/worker/JsFileWrapping"),
      ...metroTransformPlugins.getTransformPluginCacheKeyFiles(),
    ]);
    const babelTransformer = require(babelTransformerPath);
    return [
      filesKey,
      stableHash(remainingConfig).toString("hex"),
      babelTransformer.getCacheKey ? babelTransformer.getCacheKey() : "",
    ].join("$");
  },
};
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
