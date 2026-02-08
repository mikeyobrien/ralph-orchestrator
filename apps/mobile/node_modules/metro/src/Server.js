"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _Assets = require("./Assets");
var _baseJSBundle = _interopRequireDefault(
  require("./DeltaBundler/Serializers/baseJSBundle"),
);
var _getAllFiles = _interopRequireDefault(
  require("./DeltaBundler/Serializers/getAllFiles"),
);
var _getAssets = _interopRequireDefault(
  require("./DeltaBundler/Serializers/getAssets"),
);
var _getExplodedSourceMap = require("./DeltaBundler/Serializers/getExplodedSourceMap");
var _getRamBundleInfo = _interopRequireDefault(
  require("./DeltaBundler/Serializers/getRamBundleInfo"),
);
var _sourceMapString = require("./DeltaBundler/Serializers/sourceMapString");
var _IncrementalBundler = _interopRequireDefault(
  require("./IncrementalBundler"),
);
var _ResourceNotFoundError = _interopRequireDefault(
  require("./IncrementalBundler/ResourceNotFoundError"),
);
var _bundleToString = _interopRequireDefault(require("./lib/bundleToString"));
var _formatBundlingError = _interopRequireDefault(
  require("./lib/formatBundlingError"),
);
var _getGraphId = _interopRequireDefault(require("./lib/getGraphId"));
var _parseBundleOptionsFromBundleRequestUrl = _interopRequireDefault(
  require("./lib/parseBundleOptionsFromBundleRequestUrl"),
);
var _parseJsonBody = _interopRequireDefault(require("./lib/parseJsonBody"));
var _splitBundleOptions = _interopRequireDefault(
  require("./lib/splitBundleOptions"),
);
var transformHelpers = _interopRequireWildcard(
  require("./lib/transformHelpers"),
);
var _ModuleResolution = require("./node-haste/DependencyGraph/ModuleResolution");
var _parsePlatformFilePath = _interopRequireDefault(
  require("./node-haste/lib/parsePlatformFilePath"),
);
var _MultipartResponse = _interopRequireDefault(
  require("./Server/MultipartResponse"),
);
var _symbolicate = _interopRequireDefault(require("./Server/symbolicate"));
var _types = require("./shared/types");
var _codeFrame = require("@babel/code-frame");
var fs = _interopRequireWildcard(require("graceful-fs"));
var _invariant = _interopRequireDefault(require("invariant"));
var jscSafeUrl = _interopRequireWildcard(require("jsc-safe-url"));
var _metroCore = require("metro-core");
var _mimeTypes = _interopRequireDefault(require("mime-types"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
var _path = _interopRequireDefault(require("path"));
var _perf_hooks = require("perf_hooks");
var _querystring = _interopRequireDefault(require("querystring"));
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
const debug = require("debug")("Metro:Server");
const { createActionStartEntry, createActionEndEntry, log } = _metroCore.Logger;
const noopLogger = {
  start: () => {},
  point: () => {},
  annotate: () => {},
  subSpan: () => noopLogger,
  end: () => {},
};
const DELTA_ID_HEADER = "X-Metro-Delta-ID";
const FILES_CHANGED_COUNT_HEADER = "X-Metro-Files-Changed-Count";
class Server {
  constructor(config, options) {
    this._config = config;
    this._serverOptions = options;
    if (this._config.resetCache) {
      this._config.cacheStores.forEach((store) => store.clear());
      this._config.reporter.update({
        type: "transform_cache_reset",
      });
    }
    this._reporter = config.reporter;
    this._logger = _metroCore.Logger;
    this._platforms = new Set(this._config.resolver.platforms);
    this._allowedSuffixesForSourceRequests = [
      ...new Set(
        [
          ...this._config.resolver.sourceExts,
          ...this._config.watcher.additionalExts,
          ...this._config.resolver.assetExts,
        ].map((ext) => "." + ext),
      ),
    ];
    this._sourceRequestRoutingMap = [
      ["/[metro-project]/", _path.default.resolve(this._config.projectRoot)],
      ...this._config.watchFolders.map((watchFolder, index) => [
        `/[metro-watchFolders]/${index}/`,
        _path.default.resolve(watchFolder),
      ]),
    ];
    this._isEnded = false;
    this._createModuleId = config.serializer.createModuleIdFactory();
    this._bundler = new _IncrementalBundler.default(config, {
      hasReducedPerformance: options && options.hasReducedPerformance,
      watch: options ? options.watch : undefined,
    });
    this._nextBundleBuildNumber = 1;
  }
  async end() {
    if (!this._isEnded) {
      await this._bundler.end();
      this._isEnded = true;
    }
  }
  getBundler() {
    return this._bundler;
  }
  getCreateModuleId() {
    return this._createModuleId;
  }
  async _serializeGraph({ splitOptions, prepend, graph }) {
    const {
      entryFile,
      graphOptions,
      resolverOptions,
      serializerOptions,
      transformOptions,
    } = splitOptions;
    const entryPoint = this._getEntryPointAbsolutePath(entryFile);
    const bundleOptions = {
      asyncRequireModulePath: await this._resolveRelativePath(
        this._config.transformer.asyncRequireModulePath,
        {
          relativeTo: "project",
          resolverOptions,
          transformOptions,
        },
      ),
      processModuleFilter: this._config.serializer.processModuleFilter,
      createModuleId: this._createModuleId,
      getRunModuleStatement: this._config.serializer.getRunModuleStatement,
      globalPrefix: this._config.transformer.globalPrefix,
      dev: transformOptions.dev,
      includeAsyncPaths: graphOptions.lazy,
      projectRoot: this._config.projectRoot,
      modulesOnly: serializerOptions.modulesOnly,
      runBeforeMainModule:
        this._config.serializer.getModulesRunBeforeMainModule(
          _path.default.relative(this._config.projectRoot, entryPoint),
        ),
      runModule: serializerOptions.runModule,
      sourceMapUrl: serializerOptions.sourceMapUrl,
      sourceUrl: serializerOptions.sourceUrl,
      inlineSourceMap: serializerOptions.inlineSourceMap,
      serverRoot:
        this._config.server.unstable_serverRoot ?? this._config.projectRoot,
      shouldAddToIgnoreList: (module) =>
        this._shouldAddModuleToIgnoreList(module),
      getSourceUrl: (module) =>
        this._getModuleSourceUrl(module, serializerOptions.sourcePaths),
    };
    let bundleCode = null;
    let bundleMap = null;
    if (this._config.serializer.customSerializer) {
      const bundle = await this._config.serializer.customSerializer(
        entryPoint,
        prepend,
        graph,
        bundleOptions,
      );
      if (typeof bundle === "string") {
        bundleCode = bundle;
      } else {
        bundleCode = bundle.code;
        bundleMap = bundle.map;
      }
    } else {
      bundleCode = (0, _bundleToString.default)(
        (0, _baseJSBundle.default)(entryPoint, prepend, graph, bundleOptions),
      ).code;
    }
    if (!bundleMap) {
      bundleMap = await (0, _sourceMapString.sourceMapStringNonBlocking)(
        [...prepend, ...this._getSortedModules(graph)],
        {
          excludeSource: serializerOptions.excludeSource,
          processModuleFilter: this._config.serializer.processModuleFilter,
          shouldAddToIgnoreList: bundleOptions.shouldAddToIgnoreList,
          getSourceUrl: (module) =>
            this._getModuleSourceUrl(module, serializerOptions.sourcePaths),
        },
      );
    }
    return {
      code: bundleCode,
      map: bundleMap,
    };
  }
  async build(bundleOptions, { withAssets } = {}) {
    const splitOptions = (0, _splitBundleOptions.default)(bundleOptions);
    const {
      entryFile,
      graphOptions,
      onProgress,
      resolverOptions,
      transformOptions,
    } = splitOptions;
    const { prepend, graph } = await this._bundler.buildGraph(
      entryFile,
      transformOptions,
      resolverOptions,
      {
        onProgress,
        shallow: graphOptions.shallow,
        lazy: graphOptions.lazy,
      },
    );
    const [{ code, map }, assets] = await Promise.all([
      this._serializeGraph({
        splitOptions,
        prepend,
        graph,
      }),
      withAssets
        ? this._getAssetsFromDependencies(
            graph.dependencies,
            bundleOptions.platform,
          )
        : null,
    ]);
    return {
      code,
      map,
      ...(withAssets
        ? {
            assets: (0, _nullthrows.default)(assets),
          }
        : null),
    };
  }
  async getRamBundleInfo(options) {
    const {
      entryFile,
      graphOptions,
      onProgress,
      resolverOptions,
      serializerOptions,
      transformOptions,
    } = (0, _splitBundleOptions.default)(options);
    const { prepend, graph } = await this._bundler.buildGraph(
      entryFile,
      transformOptions,
      resolverOptions,
      {
        onProgress,
        shallow: graphOptions.shallow,
        lazy: graphOptions.lazy,
      },
    );
    const entryPoint = this._getEntryPointAbsolutePath(entryFile);
    return await (0, _getRamBundleInfo.default)(entryPoint, prepend, graph, {
      asyncRequireModulePath: await this._resolveRelativePath(
        this._config.transformer.asyncRequireModulePath,
        {
          relativeTo: "project",
          resolverOptions,
          transformOptions,
        },
      ),
      processModuleFilter: this._config.serializer.processModuleFilter,
      createModuleId: this._createModuleId,
      dev: transformOptions.dev,
      excludeSource: serializerOptions.excludeSource,
      getRunModuleStatement: this._config.serializer.getRunModuleStatement,
      getTransformOptions: this._config.transformer.getTransformOptions,
      globalPrefix: this._config.transformer.globalPrefix,
      includeAsyncPaths: graphOptions.lazy,
      platform: transformOptions.platform,
      projectRoot: this._config.projectRoot,
      modulesOnly: serializerOptions.modulesOnly,
      runBeforeMainModule:
        this._config.serializer.getModulesRunBeforeMainModule(
          _path.default.relative(this._config.projectRoot, entryPoint),
        ),
      runModule: serializerOptions.runModule,
      sourceMapUrl: serializerOptions.sourceMapUrl,
      sourceUrl: serializerOptions.sourceUrl,
      inlineSourceMap: serializerOptions.inlineSourceMap,
      serverRoot:
        this._config.server.unstable_serverRoot ?? this._config.projectRoot,
      shouldAddToIgnoreList: (module) =>
        this._shouldAddModuleToIgnoreList(module),
      getSourceUrl: (module) =>
        this._getModuleSourceUrl(module, serializerOptions.sourcePaths),
    });
  }
  async getAssets(options) {
    const { entryFile, onProgress, resolverOptions, transformOptions } = (0,
    _splitBundleOptions.default)(options);
    const dependencies = await this._bundler.getDependencies(
      [entryFile],
      transformOptions,
      resolverOptions,
      {
        onProgress,
        shallow: false,
        lazy: false,
      },
    );
    return this._getAssetsFromDependencies(
      dependencies,
      transformOptions.platform,
    );
  }
  async _getAssetsFromDependencies(dependencies, platform) {
    return await (0, _getAssets.default)(dependencies, {
      processModuleFilter: this._config.serializer.processModuleFilter,
      assetPlugins: this._config.transformer.assetPlugins,
      platform,
      projectRoot: this._getServerRootDir(),
      publicPath: this._config.transformer.publicPath,
    });
  }
  async getOrderedDependencyPaths(options) {
    const { entryFile, onProgress, resolverOptions, transformOptions } = (0,
    _splitBundleOptions.default)({
      ...Server.DEFAULT_BUNDLE_OPTIONS,
      ...options,
    });
    const { prepend, graph } = await this._bundler.buildGraph(
      entryFile,
      transformOptions,
      resolverOptions,
      {
        onProgress,
        shallow: false,
        lazy: false,
      },
    );
    const platform =
      transformOptions.platform ||
      (0, _parsePlatformFilePath.default)(entryFile, this._platforms).platform;
    return await (0, _getAllFiles.default)(prepend, graph, {
      platform,
      processModuleFilter: this._config.serializer.processModuleFilter,
    });
  }
  _rangeRequestMiddleware(req, res, data, assetPath) {
    if (req.headers && req.headers.range) {
      const [rangeStart, rangeEnd] = req.headers.range
        .replace(/bytes=/, "")
        .split("-");
      const dataStart = parseInt(rangeStart, 10);
      const dataEnd = rangeEnd ? parseInt(rangeEnd, 10) : data.length - 1;
      const chunksize = dataEnd - dataStart + 1;
      res.writeHead(206, {
        "Accept-Ranges": "bytes",
        "Content-Length": chunksize.toString(),
        "Content-Range": `bytes ${dataStart}-${dataEnd}/${data.length}`,
      });
      return data.slice(dataStart, dataEnd + 1);
    }
    res.setHeader("Content-Length", String(Buffer.byteLength(data)));
    return data;
  }
  async _processSingleAssetRequest(req, res) {
    debug("Processing single asset request: %s", req.url);
    if (!URL.canParse(req.url, "resolve://")) {
      throw new Error("Could not parse URL", {
        cause: req.url,
      });
    }
    const urlObj = new URL(req.url, "resolve://");
    const formattedUrl = urlObj.toString();
    if (req.url !== formattedUrl) {
      debug("Formatted as:    %s", formattedUrl);
    }
    let [, assetPath] =
      urlObj.pathname
        .split("/")
        .map((segment) => decodeURIComponent(segment))
        .join("/")
        .match(/^\/assets\/(.+)$/) || [];
    if (!assetPath && urlObj.searchParams.get("unstable_path")) {
      const [, actualPath, secondaryQuery] = (0, _nullthrows.default)(
        (urlObj.searchParams.get("unstable_path") || "").match(
          /^([^?]*)\??(.*)$/,
        ),
      );
      if (secondaryQuery) {
        Object.entries(_querystring.default.parse(secondaryQuery)).forEach(
          ([key, value]) => {
            urlObj.searchParams.set(key, value);
          },
        );
      }
      assetPath = actualPath;
    }
    if (!assetPath) {
      throw new Error("Could not extract asset path from URL");
    }
    const processingAssetRequestLogEntry = log(
      createActionStartEntry({
        action_name: "Processing asset request",
        asset: assetPath[1],
      }),
    );
    try {
      const data = await (0, _Assets.getAsset)(
        assetPath,
        this._config.projectRoot,
        this._config.watchFolders,
        urlObj.searchParams.get("platform"),
        this._config.resolver.assetExts,
      );
      if (process.env.REACT_NATIVE_ENABLE_ASSET_CACHING === true) {
        res.setHeader("Cache-Control", "max-age=31536000");
      }
      res.setHeader(
        "Content-Type",
        _mimeTypes.default.lookup(_path.default.basename(assetPath)),
      );
      res.end(this._rangeRequestMiddleware(req, res, data, assetPath));
      process.nextTick(() => {
        log(createActionEndEntry(processingAssetRequestLogEntry));
      });
    } catch (error) {
      console.error(error.stack);
      res.writeHead(404);
      res.end("Asset not found");
    }
  }
  processRequest = (req, res, next) => {
    this._processRequest(req, res, next).catch(next);
  };
  _parseOptions(url) {
    const { bundleType: _bundleType, ...bundleOptions } = (0,
    _parseBundleOptionsFromBundleRequestUrl.default)(
      url,
      new Set(this._config.resolver.platforms),
    );
    return bundleOptions;
  }
  _rewriteAndNormalizeUrl(requestUrl) {
    return jscSafeUrl.toNormalUrl(
      this._config.server.rewriteRequestUrl(jscSafeUrl.toNormalUrl(requestUrl)),
    );
  }
  async _processRequest(req, res, next) {
    const originalUrl = req.url;
    debug("Handling request:    %s", originalUrl);
    req.url = this._rewriteAndNormalizeUrl(req.url);
    if (req.url !== originalUrl) {
      debug("Rewritten to:    %s", req.url);
    }
    const reqHost = req.headers["x-forwarded-host"] || req.headers["host"];
    if (!reqHost) {
      throw new Error("No host header was found.");
    }
    const reqProtocol =
      req.headers["x-forwarded-proto"] ||
      (req.socket?.encrypted === true ? "https" : "http");
    const urlObj = new URL(req.url, reqProtocol + "://" + reqHost);
    const formattedUrl = urlObj.toString();
    if (req.url !== formattedUrl) {
      debug("Formatted as:    %s", formattedUrl);
    }
    const pathname = urlObj.pathname || "";
    const filePathname = pathname
      .split("/")
      .map((segment) => decodeURIComponent(segment))
      .join("/");
    const buildNumber = this.getNewBuildNumber();
    if (pathname.endsWith(".bundle")) {
      const options = this._parseOptions(formattedUrl);
      await this._processBundleRequest(req, res, options, {
        buildNumber,
        bundlePerfLogger:
          this._config.unstable_perfLoggerFactory?.("BUNDLING_REQUEST", {
            key: buildNumber,
          }) ?? noopLogger,
      });
      if (this._serverOptions && this._serverOptions.onBundleBuilt) {
        this._serverOptions.onBundleBuilt(filePathname);
      }
    } else if (pathname.endsWith(".map")) {
      res.setHeader("Access-Control-Allow-Origin", "devtools://devtools");
      await this._processSourceMapRequest(
        req,
        res,
        this._parseOptions(formattedUrl),
        {
          buildNumber,
          bundlePerfLogger: noopLogger,
        },
      );
    } else if (pathname.endsWith(".assets")) {
      await this._processAssetsRequest(
        req,
        res,
        this._parseOptions(formattedUrl),
        {
          buildNumber,
          bundlePerfLogger: noopLogger,
        },
      );
    } else if (pathname.startsWith("/assets/") || pathname === "/assets") {
      await this._processSingleAssetRequest(req, res);
    } else if (pathname === "/symbolicate") {
      await this._symbolicate(req, res);
    } else {
      let handled = false;
      for (const [pathnamePrefix, normalizedRootDir] of this
        ._sourceRequestRoutingMap) {
        if (filePathname.startsWith(pathnamePrefix)) {
          const relativeFilePathname = filePathname.substr(
            pathnamePrefix.length,
          );
          await this._processSourceRequest(
            relativeFilePathname,
            normalizedRootDir,
            res,
          );
          handled = true;
          break;
        }
      }
      if (!handled) {
        next();
      }
    }
  }
  async _processSourceRequest(relativeFilePathname, rootDir, res) {
    if (
      !this._allowedSuffixesForSourceRequests.some((suffix) =>
        relativeFilePathname.endsWith(suffix),
      )
    ) {
      res.writeHead(404);
      res.end();
      return;
    }
    const depGraph = await this._bundler.getBundler().getDependencyGraph();
    const filePath = _path.default.join(rootDir, relativeFilePathname);
    try {
      await depGraph.getOrComputeSha1(filePath);
    } catch {
      res.writeHead(404);
      res.end();
      return;
    }
    const mimeType = _mimeTypes.default.lookup(
      _path.default.basename(relativeFilePathname),
    );
    res.setHeader("Content-Type", mimeType);
    const stream = fs.createReadStream(filePath);
    stream.pipe(res);
    stream.on("error", (error) => {
      if (error.code === "ENOENT") {
        res.writeHead(404);
        res.end();
      } else {
        res.writeHead(500);
        res.end();
      }
    });
  }
  _createRequestProcessor({
    bundleType,
    createStartEntry,
    createEndEntry,
    build,
    delete: deleteFn,
    finish,
  }) {
    return async function requestProcessor(
      req,
      res,
      bundleOptions,
      buildContext,
    ) {
      const requestStartTimestamp =
        _perf_hooks.performance.timeOrigin + _perf_hooks.performance.now();
      const { buildNumber } = buildContext;
      const {
        entryFile,
        graphOptions,
        resolverOptions,
        serializerOptions,
        transformOptions,
      } = (0, _splitBundleOptions.default)(bundleOptions);
      let resolvedEntryFilePath;
      try {
        resolvedEntryFilePath = await this._resolveRelativePath(entryFile, {
          relativeTo: "server",
          resolverOptions,
          transformOptions,
        });
      } catch (error) {
        const formattedError = (0, _formatBundlingError.default)(error);
        const status =
          error instanceof _ModuleResolution.UnableToResolveError ? 404 : 500;
        res.writeHead(status, {
          "Content-Type": "application/json; charset=UTF-8",
        });
        res.end(JSON.stringify(formattedError));
        return;
      }
      const graphId = (0, _getGraphId.default)(
        resolvedEntryFilePath,
        transformOptions,
        {
          unstable_allowRequireContext:
            this._config.transformer.unstable_allowRequireContext,
          resolverOptions,
          shallow: graphOptions.shallow,
          lazy: graphOptions.lazy,
        },
      );
      if (deleteFn && req.method === "DELETE") {
        const deleteContext = {
          graphId,
          req,
          res,
        };
        try {
          await deleteFn(deleteContext);
        } catch (error) {
          const formattedError = (0, _formatBundlingError.default)(error);
          const status =
            error instanceof _ResourceNotFoundError.default ? 404 : 500;
          res.writeHead(status, {
            "Content-Type": "application/json; charset=UTF-8",
          });
          res.end(JSON.stringify(formattedError));
        }
        return;
      }
      const mres = _MultipartResponse.default.wrapIfSupported(req, res);
      let onProgress = null;
      let lastProgress = -1;
      if (this._config.reporter) {
        onProgress = (transformedFileCount, totalFileCount) => {
          const currentProgress = parseInt(
            (transformedFileCount / totalFileCount) * 100,
            10,
          );
          if (currentProgress > lastProgress || totalFileCount < 10) {
            if (mres instanceof _MultipartResponse.default) {
              mres.writeChunk(
                {
                  "Content-Type": "application/json",
                },
                JSON.stringify({
                  done: transformedFileCount,
                  total: totalFileCount,
                }),
              );
            }
            if (res.socket != null && res.socket.uncork != null) {
              res.socket.uncork();
            }
            lastProgress = currentProgress;
          }
          this._reporter.update({
            buildID: getBuildID(buildNumber),
            type: "bundle_transform_progressed",
            transformedFileCount,
            totalFileCount,
          });
        };
      }
      this._reporter.update({
        buildID: getBuildID(buildNumber),
        bundleDetails: {
          bundleType,
          customResolverOptions: bundleOptions.customResolverOptions,
          customTransformOptions: bundleOptions.customTransformOptions,
          dev: transformOptions.dev,
          entryFile: resolvedEntryFilePath,
          minify: transformOptions.minify,
          platform: transformOptions.platform,
        },
        isPrefetch: req.method === "HEAD",
        type: "bundle_build_started",
      });
      const startContext = {
        buildNumber,
        bundleOptions,
        entryFile: resolvedEntryFilePath,
        graphId,
        graphOptions,
        mres,
        onProgress,
        req,
        resolverOptions,
        serializerOptions,
        transformOptions,
        bundlePerfLogger: buildContext.bundlePerfLogger,
        requestStartTimestamp,
      };
      const logEntry = log(
        createActionStartEntry(createStartEntry(startContext)),
      );
      let result;
      try {
        result = await build(startContext);
      } catch (error) {
        const formattedError = (0, _formatBundlingError.default)(error);
        const status =
          error instanceof _ResourceNotFoundError.default ? 404 : 500;
        mres.writeHead(status, {
          "Content-Type": "application/json; charset=UTF-8",
        });
        mres.end(JSON.stringify(formattedError));
        this._reporter.update({
          buildID: getBuildID(buildNumber),
          type: "bundle_build_failed",
          bundleOptions,
        });
        this._reporter.update({
          error,
          type: "bundling_error",
        });
        log({
          action_name: "bundling_error",
          error_type: formattedError.type,
          log_entry_label: "bundling_error",
          bundle_id: graphId,
          build_id: getBuildID(buildNumber),
          stack: formattedError.message,
        });
        debug("Bundling error", error);
        buildContext.bundlePerfLogger.end("FAIL");
        return;
      }
      const endContext = {
        ...startContext,
        result,
      };
      finish(endContext);
      this._reporter.update({
        buildID: getBuildID(buildNumber),
        type: "bundle_build_done",
      });
      log(
        createActionEndEntry({
          ...logEntry,
          ...createEndEntry(endContext),
        }),
      );
    };
  }
  _processBundleRequest = this._createRequestProcessor({
    bundleType: "bundle",
    createStartEntry(context) {
      return {
        action_name: "Requesting bundle",
        bundle_url: context.req.url,
        entry_point: context.entryFile,
        bundler: "delta",
        build_id: getBuildID(context.buildNumber),
        bundle_options: context.bundleOptions,
        bundle_hash: context.graphId,
        user_agent: context.req.headers["user-agent"] ?? "unknown",
      };
    },
    createEndEntry(context) {
      return {
        outdated_modules: context.result.numModifiedFiles,
      };
    },
    build: async ({
      entryFile,
      graphId,
      graphOptions,
      onProgress,
      resolverOptions,
      serializerOptions,
      transformOptions,
      bundlePerfLogger,
      requestStartTimestamp,
    }) => {
      bundlePerfLogger.start({
        timestamp: requestStartTimestamp,
      });
      bundlePerfLogger.annotate({
        string: {
          bundle_url: entryFile,
        },
      });
      const revPromise = this._bundler.getRevisionByGraphId(graphId);
      bundlePerfLogger.point("resolvingAndTransformingDependencies_start");
      bundlePerfLogger.annotate({
        bool: {
          initial_build: revPromise == null,
        },
      });
      const { delta, revision } = await (revPromise != null
        ? this._bundler.updateGraph(await revPromise, false)
        : this._bundler.initializeGraph(
            entryFile,
            transformOptions,
            resolverOptions,
            {
              onProgress,
              shallow: graphOptions.shallow,
              lazy: graphOptions.lazy,
            },
          ));
      bundlePerfLogger.annotate({
        int: {
          graph_node_count: revision.graph.dependencies.size,
        },
      });
      bundlePerfLogger.point("resolvingAndTransformingDependencies_end");
      bundlePerfLogger.point("serializingBundle_start");
      const serializer =
        this._config.serializer.customSerializer ||
        ((entryPoint, preModules, graph, options) =>
          (0, _bundleToString.default)(
            (0, _baseJSBundle.default)(entryPoint, preModules, graph, options),
          ).code);
      const bundle = await serializer(
        entryFile,
        revision.prepend,
        revision.graph,
        {
          asyncRequireModulePath: await this._resolveRelativePath(
            this._config.transformer.asyncRequireModulePath,
            {
              relativeTo: "project",
              resolverOptions,
              transformOptions,
            },
          ),
          processModuleFilter: this._config.serializer.processModuleFilter,
          createModuleId: this._createModuleId,
          getRunModuleStatement: this._config.serializer.getRunModuleStatement,
          globalPrefix: this._config.transformer.globalPrefix,
          includeAsyncPaths: graphOptions.lazy,
          dev: transformOptions.dev,
          projectRoot: this._config.projectRoot,
          modulesOnly: serializerOptions.modulesOnly,
          runBeforeMainModule:
            this._config.serializer.getModulesRunBeforeMainModule(
              _path.default.relative(this._config.projectRoot, entryFile),
            ),
          runModule: serializerOptions.runModule,
          sourceMapUrl: serializerOptions.sourceMapUrl,
          sourceUrl: serializerOptions.sourceUrl,
          inlineSourceMap: serializerOptions.inlineSourceMap,
          serverRoot:
            this._config.server.unstable_serverRoot ?? this._config.projectRoot,
          shouldAddToIgnoreList: (module) =>
            this._shouldAddModuleToIgnoreList(module),
          getSourceUrl: (module) =>
            this._getModuleSourceUrl(module, serializerOptions.sourcePaths),
        },
      );
      bundlePerfLogger.point("serializingBundle_end");
      const bundleCode = typeof bundle === "string" ? bundle : bundle.code;
      return {
        numModifiedFiles: delta.reset
          ? delta.added.size + revision.prepend.length
          : delta.added.size + delta.modified.size + delta.deleted.size,
        lastModifiedDate: revision.date,
        nextRevId: revision.id,
        bundle: bundleCode,
      };
    },
    finish({ req, mres, serializerOptions, result, bundlePerfLogger }) {
      bundlePerfLogger.annotate({
        int: {
          bundle_length: result.bundle.length,
          bundle_byte_length: Buffer.byteLength(result.bundle),
        },
      });
      mres.once("error", () => {
        bundlePerfLogger.end("FAIL");
      });
      mres.once("finish", () => {
        bundlePerfLogger.end("SUCCESS");
      });
      if (
        req.headers["if-modified-since"] ===
        result.lastModifiedDate.toUTCString()
      ) {
        bundlePerfLogger.annotate({
          string: {
            http_status: "304",
          },
        });
        debug("Responding with 304");
        mres.writeHead(304);
        mres.end();
      } else {
        bundlePerfLogger.annotate({
          string: {
            http_status: "200",
          },
        });
        mres.setHeader(
          FILES_CHANGED_COUNT_HEADER,
          String(result.numModifiedFiles),
        );
        mres.setHeader(DELTA_ID_HEADER, String(result.nextRevId));
        if (serializerOptions?.sourceUrl != null) {
          mres.setHeader("Content-Location", serializerOptions.sourceUrl);
        }
        mres.setHeader("Content-Type", "application/javascript; charset=UTF-8");
        mres.setHeader("Last-Modified", result.lastModifiedDate.toUTCString());
        mres.setHeader(
          "Content-Length",
          String(Buffer.byteLength(result.bundle)),
        );
        mres.end(result.bundle);
      }
    },
    delete: async ({ graphId, res }) => {
      await this._bundler.endGraph(graphId);
      res.statusCode = 204;
      res.end();
    },
  });
  _getSortedModules(graph) {
    const modules = [...graph.dependencies.values()];
    for (const module of modules) {
      this._createModuleId(module.path);
    }
    return modules.sort(
      (a, b) => this._createModuleId(a.path) - this._createModuleId(b.path),
    );
  }
  _processSourceMapRequest = this._createRequestProcessor({
    bundleType: "map",
    createStartEntry(context) {
      return {
        action_name: "Requesting sourcemap",
        bundle_url: context.req.url,
        entry_point: context.entryFile,
        bundler: "delta",
      };
    },
    createEndEntry(context) {
      return {
        bundler: "delta",
      };
    },
    build: async ({
      entryFile,
      graphId,
      graphOptions,
      onProgress,
      resolverOptions,
      serializerOptions,
      transformOptions,
    }) => {
      let revision;
      const revPromise = this._bundler.getRevisionByGraphId(graphId);
      if (revPromise == null) {
        ({ revision } = await this._bundler.initializeGraph(
          entryFile,
          transformOptions,
          resolverOptions,
          {
            onProgress,
            shallow: graphOptions.shallow,
            lazy: graphOptions.lazy,
          },
        ));
      } else {
        ({ revision } = await this._bundler.updateGraph(
          await revPromise,
          false,
        ));
      }
      let { prepend, graph } = revision;
      if (serializerOptions.modulesOnly) {
        prepend = [];
      }
      return await (0, _sourceMapString.sourceMapStringNonBlocking)(
        [...prepend, ...this._getSortedModules(graph)],
        {
          excludeSource: serializerOptions.excludeSource,
          processModuleFilter: this._config.serializer.processModuleFilter,
          shouldAddToIgnoreList: (module) =>
            this._shouldAddModuleToIgnoreList(module),
          getSourceUrl: (module) =>
            this._getModuleSourceUrl(module, serializerOptions.sourcePaths),
        },
      );
    },
    finish({ mres, result }) {
      mres.setHeader("Content-Type", "application/json");
      mres.end(result.toString());
    },
  });
  _processAssetsRequest = this._createRequestProcessor({
    bundleType: "assets",
    createStartEntry(context) {
      return {
        action_name: "Requesting assets",
        bundle_url: context.req.url,
        entry_point: context.entryFile,
        bundler: "delta",
      };
    },
    createEndEntry(context) {
      return {
        bundler: "delta",
      };
    },
    build: async ({
      entryFile,
      onProgress,
      resolverOptions,
      transformOptions,
    }) => {
      const dependencies = await this._bundler.getDependencies(
        [entryFile],
        transformOptions,
        resolverOptions,
        {
          onProgress,
          shallow: false,
          lazy: false,
        },
      );
      return await (0, _getAssets.default)(dependencies, {
        processModuleFilter: this._config.serializer.processModuleFilter,
        assetPlugins: this._config.transformer.assetPlugins,
        platform: transformOptions.platform,
        publicPath: this._config.transformer.publicPath,
        projectRoot: this._config.projectRoot,
      });
    },
    finish({ mres, result }) {
      mres.setHeader("Content-Type", "application/json");
      mres.end(JSON.stringify(result));
    },
  });
  async _symbolicate(req, res) {
    const getCodeFrame = (urls, symbolicatedStack) => {
      const allFramesCollapsed = symbolicatedStack.every(
        ({ collapse }) => collapse,
      );
      for (let i = 0; i < symbolicatedStack.length; i++) {
        const { collapse, column, file, lineNumber } = symbolicatedStack[i];
        if (
          (!allFramesCollapsed && collapse) ||
          lineNumber == null ||
          (file != null && urls.has(file))
        ) {
          continue;
        }
        const fileAbsolute = _path.default.resolve(
          this._config.projectRoot,
          file ?? "",
        );
        try {
          return {
            content: (0, _codeFrame.codeFrameColumns)(
              fs.readFileSync(fileAbsolute, "utf8"),
              {
                start: {
                  column: column + 1,
                  line: lineNumber,
                },
              },
              {
                forceColor: true,
              },
            ),
            location: {
              row: lineNumber,
              column,
            },
            fileName: file,
          };
        } catch (error) {
          debug(
            "Generating code frame failed on file read.",
            fileAbsolute,
            error,
          );
        }
      }
      return null;
    };
    try {
      const symbolicatingLogEntry = log(
        createActionStartEntry("Symbolicating"),
      );
      debug("Start symbolication");
      let parsedBody;
      if ("rawBody" in req) {
        const body = await req.rawBody;
        parsedBody = JSON.parse(body);
      } else {
        parsedBody = await (0, _parseJsonBody.default)(req);
      }
      const rewriteAndNormalizeStackFrame = (frame, lineNumber) => {
        (0, _invariant.default)(
          frame != null && typeof frame === "object",
          "Bad stack frame at line %d, expected object, received: %s",
          lineNumber,
          typeof frame,
        );
        const frameFile = frame.file;
        if (typeof frameFile === "string" && frameFile.includes("://")) {
          return {
            ...frame,
            file: this._rewriteAndNormalizeUrl(frameFile),
          };
        }
        return frame;
      };
      const stack = parsedBody.stack.map(rewriteAndNormalizeStackFrame);
      const urls = new Set();
      stack.forEach((frame) => {
        const sourceUrl = frame.file;
        if (
          sourceUrl != null &&
          !urls.has(sourceUrl) &&
          !sourceUrl.endsWith("/debuggerWorker.js") &&
          sourceUrl.startsWith("http")
        ) {
          urls.add(sourceUrl);
        }
      });
      debug("Getting source maps for symbolication");
      const sourceMaps = await Promise.all(
        Array.from(urls.values()).map((normalizedUrl) =>
          this._explodedSourceMapForBundleOptions(
            this._parseOptions(normalizedUrl),
          ),
        ),
      );
      debug("Performing fast symbolication");
      const symbolicatedStack = await (0, _symbolicate.default)(
        stack,
        zip(urls.values(), sourceMaps),
        this._config,
        parsedBody.extraData ?? {},
      );
      debug("Symbolication done");
      res.end(
        JSON.stringify({
          codeFrame: getCodeFrame(urls, symbolicatedStack),
          stack: symbolicatedStack,
        }),
      );
      process.nextTick(() => {
        log(createActionEndEntry(symbolicatingLogEntry));
      });
    } catch (error) {
      debug("Symbolication failed", error.stack || error);
      res.statusCode = 500;
      res.end(
        JSON.stringify({
          error: error.message,
        }),
      );
    }
  }
  async _explodedSourceMapForBundleOptions(bundleOptions) {
    const {
      entryFile,
      graphOptions,
      onProgress,
      resolverOptions,
      serializerOptions,
      transformOptions,
    } = (0, _splitBundleOptions.default)(bundleOptions);
    const resolvedEntryFilePath = await this._resolveRelativePath(entryFile, {
      relativeTo: "server",
      resolverOptions,
      transformOptions,
    });
    const graphId = (0, _getGraphId.default)(
      resolvedEntryFilePath,
      transformOptions,
      {
        unstable_allowRequireContext:
          this._config.transformer.unstable_allowRequireContext,
        resolverOptions,
        shallow: graphOptions.shallow,
        lazy: graphOptions.lazy,
      },
    );
    let revision;
    const revPromise = this._bundler.getRevisionByGraphId(graphId);
    if (revPromise == null) {
      ({ revision } = await this._bundler.initializeGraph(
        resolvedEntryFilePath,
        transformOptions,
        resolverOptions,
        {
          onProgress,
          shallow: graphOptions.shallow,
          lazy: graphOptions.lazy,
        },
      ));
    } else {
      ({ revision } = await this._bundler.updateGraph(await revPromise, false));
    }
    let { prepend, graph } = revision;
    if (serializerOptions.modulesOnly) {
      prepend = [];
    }
    return (0, _getExplodedSourceMap.getExplodedSourceMap)(
      [...prepend, ...this._getSortedModules(graph)],
      {
        processModuleFilter: this._config.serializer.processModuleFilter,
      },
    );
  }
  async _resolveRelativePath(
    filePath,
    { relativeTo, resolverOptions, transformOptions },
  ) {
    const resolutionFn = await transformHelpers.getResolveDependencyFn(
      this._bundler.getBundler(),
      transformOptions.platform,
      resolverOptions,
    );
    const rootDir =
      relativeTo === "server"
        ? this._getServerRootDir()
        : this._config.projectRoot;
    return resolutionFn(`${rootDir}/.`, {
      name: filePath,
      data: {
        key: filePath,
        locs: [],
        asyncType: null,
        isESMImport: false,
      },
    }).filePath;
  }
  getNewBuildNumber() {
    return this._nextBundleBuildNumber++;
  }
  getPlatforms() {
    return this._config.resolver.platforms;
  }
  getWatchFolders() {
    return this._config.watchFolders;
  }
  static DEFAULT_GRAPH_OPTIONS = {
    customResolverOptions: Object.create(null),
    customTransformOptions: Object.create(null),
    dev: true,
    minify: false,
    unstable_transformProfile: "default",
  };
  static DEFAULT_BUNDLE_OPTIONS = {
    ...Server.DEFAULT_GRAPH_OPTIONS,
    excludeSource: false,
    inlineSourceMap: false,
    lazy: false,
    modulesOnly: false,
    onProgress: null,
    runModule: true,
    shallow: false,
    sourceMapUrl: null,
    sourceUrl: null,
    sourcePaths: _types.SourcePathsMode.Absolute,
  };
  _getServerRootDir() {
    return this._config.server.unstable_serverRoot ?? this._config.projectRoot;
  }
  _getEntryPointAbsolutePath(entryFile) {
    return _path.default.resolve(this._getServerRootDir(), entryFile);
  }
  async ready() {
    await this._bundler.ready();
  }
  _shouldAddModuleToIgnoreList(module) {
    return (
      module.path === "__prelude__" ||
      module.path.includes("?ctx=") ||
      this._config.serializer.isThirdPartyModule(module)
    );
  }
  _getModuleSourceUrl(module, mode) {
    switch (mode) {
      case _types.SourcePathsMode.ServerUrl:
        for (const [pathnamePrefix, normalizedRootDir] of this
          ._sourceRequestRoutingMap) {
          if (module.path.startsWith(normalizedRootDir + _path.default.sep)) {
            const relativePath = module.path.slice(
              normalizedRootDir.length + 1,
            );
            const relativePathPosix = relativePath
              .split(_path.default.sep)
              .map((segment) => encodeURIComponent(segment))
              .join("/");
            return pathnamePrefix + relativePathPosix;
          }
        }
        const modulePathPosix = module.path
          .split(_path.default.sep)
          .map((segment) => encodeURIComponent(segment))
          .join("/");
        return modulePathPosix.startsWith("/")
          ? modulePathPosix
          : "/" + modulePathPosix;
      case _types.SourcePathsMode.Absolute:
        return module.path;
    }
  }
}
exports.default = Server;
function* zip(xs, ys) {
  const ysIter = ys[Symbol.iterator]();
  for (const x of xs) {
    const y = ysIter.next();
    if (y.done) {
      return;
    }
    yield [x, y.value];
  }
}
function getBuildID(buildNumber) {
  return buildNumber.toString(36);
}
