"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
Object.defineProperty(exports, "JsonReporter", {
  enumerable: true,
  get: function () {
    return _JsonReporter.default;
  },
});
Object.defineProperty(exports, "Terminal", {
  enumerable: true,
  get: function () {
    return _metroCore.Terminal;
  },
});
Object.defineProperty(exports, "TerminalReporter", {
  enumerable: true,
  get: function () {
    return _TerminalReporter.default;
  },
});
exports.default =
  exports.createConnectMiddleware =
  exports.buildGraph =
  exports.attachMetroCli =
    void 0;
Object.defineProperty(exports, "loadConfig", {
  enumerable: true,
  get: function () {
    return _metroConfig.loadConfig;
  },
});
Object.defineProperty(exports, "mergeConfig", {
  enumerable: true,
  get: function () {
    return _metroConfig.mergeConfig;
  },
});
Object.defineProperty(exports, "resolveConfig", {
  enumerable: true,
  get: function () {
    return _metroConfig.resolveConfig;
  },
});
exports.runBuild = void 0;
exports.runMetro = runMetro;
exports.runServer = void 0;
var _build = _interopRequireDefault(require("./commands/build"));
var _dependencies = _interopRequireDefault(require("./commands/dependencies"));
var _serve = _interopRequireDefault(require("./commands/serve"));
var _HmrServer = _interopRequireDefault(require("./HmrServer"));
var _IncrementalBundler = _interopRequireDefault(
  require("./IncrementalBundler"),
);
var _createWebsocketServer = _interopRequireDefault(
  require("./lib/createWebsocketServer"),
);
var _JsonReporter = _interopRequireDefault(require("./lib/JsonReporter"));
var _TerminalReporter = _interopRequireDefault(
  require("./lib/TerminalReporter"),
);
var _Server = _interopRequireDefault(require("./Server"));
var outputBundle = _interopRequireWildcard(require("./shared/output/bundle"));
var _chalk = _interopRequireDefault(require("chalk"));
var _fs = _interopRequireDefault(require("fs"));
var _http = _interopRequireDefault(require("http"));
var _https = _interopRequireDefault(require("https"));
var _metroConfig = require("metro-config");
var _metroCore = require("metro-core");
var _net = _interopRequireDefault(require("net"));
var _nullthrows = _interopRequireDefault(require("nullthrows"));
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
const DEFAULTS = _Server.default.DEFAULT_BUNDLE_OPTIONS;
async function getConfig(config) {
  const defaultConfig = await (0, _metroConfig.getDefaultConfig)(
    config.projectRoot,
  );
  return (0, _metroConfig.mergeConfig)(defaultConfig, config);
}
async function runMetro(config, options) {
  const mergedConfig = await getConfig(config);
  const {
    reporter,
    server: { port },
  } = mergedConfig;
  reporter.update({
    hasReducedPerformance: options
      ? Boolean(options.hasReducedPerformance)
      : false,
    port,
    type: "initialize_started",
  });
  const { waitForBundler = false, ...serverOptions } = options ?? {};
  const server = new _Server.default(mergedConfig, serverOptions);
  const readyPromise = server
    .ready()
    .then(() => {
      reporter.update({
        type: "initialize_done",
        port,
      });
    })
    .catch((error) => {
      reporter.update({
        type: "initialize_failed",
        port,
        error,
      });
    });
  if (waitForBundler) {
    await readyPromise;
  }
  return server;
}
const createConnectMiddleware = async function (config, options) {
  const metroServer = await runMetro(config, options);
  let enhancedMiddleware = metroServer.processRequest;
  if (config.server.enhanceMiddleware) {
    enhancedMiddleware = config.server.enhanceMiddleware(
      enhancedMiddleware,
      metroServer,
    );
  }
  return {
    attachHmrServer(httpServer) {
      const wss = (0, _createWebsocketServer.default)({
        websocketServer: new _HmrServer.default(
          metroServer.getBundler(),
          metroServer.getCreateModuleId(),
          config,
        ),
      });
      httpServer.on("upgrade", (request, socket, head) => {
        const { pathname } = new URL(request.url, "resolve://");
        if (pathname === "/hot") {
          wss.handleUpgrade(request, socket, head, (ws) => {
            wss.emit("connection", ws, request);
          });
        } else {
          socket.destroy();
        }
      });
    },
    metroServer,
    middleware: enhancedMiddleware,
    async end() {
      await metroServer.end();
    },
  };
};
exports.createConnectMiddleware = createConnectMiddleware;
const runServer = async (
  config,
  {
    hasReducedPerformance = false,
    host,
    onError,
    onReady,
    onClose,
    secureServerOptions,
    secure,
    secureCert,
    secureKey,
    unstable_extraMiddleware,
    waitForBundler = false,
    websocketEndpoints = {},
    watch,
  } = {},
) => {
  await earlyPortCheck(host, config.server.port);
  if (secure != null || secureCert != null || secureKey != null) {
    console.warn(
      _chalk.default.inverse.yellow.bold(" DEPRECATED "),
      "The `secure`, `secureCert`, and `secureKey` options are now deprecated. " +
        "Please use the `secureServerOptions` object instead to pass options to " +
        "Metro's https development server.",
    );
  }
  const connect = require("connect");
  const serverApp = connect();
  const {
    middleware,
    end: endMiddleware,
    metroServer,
  } = await createConnectMiddleware(config, {
    hasReducedPerformance,
    waitForBundler,
    watch,
  });
  for (const handler of unstable_extraMiddleware ?? []) {
    serverApp.use(handler);
  }
  serverApp.use(middleware);
  let httpServer;
  if (secure || secureServerOptions != null) {
    let options = secureServerOptions;
    if (typeof secureKey === "string" && typeof secureCert === "string") {
      options = {
        key: _fs.default.readFileSync(secureKey),
        cert: _fs.default.readFileSync(secureCert),
        ...secureServerOptions,
      };
    }
    httpServer = _https.default.createServer(options, serverApp);
  } else {
    httpServer = _http.default.createServer(serverApp);
  }
  return new Promise((resolve, reject) => {
    httpServer.on("error", (error) => {
      endMiddleware().finally(() => {
        onError?.(error);
        reject(error);
      });
    });
    httpServer.listen(config.server.port, host, () => {
      const { address, port, family } = httpServer.address();
      config.reporter.update({
        type: "server_listening",
        address,
        port,
        family,
      });
      websocketEndpoints = {
        ...websocketEndpoints,
        "/hot": (0, _createWebsocketServer.default)({
          websocketServer: new _HmrServer.default(
            metroServer.getBundler(),
            metroServer.getCreateModuleId(),
            config,
          ),
        }),
      };
      httpServer.on("upgrade", (request, socket, head) => {
        const { pathname } = new URL(request.url, "resolve://");
        if (pathname != null && websocketEndpoints[pathname]) {
          websocketEndpoints[pathname].handleUpgrade(
            request,
            socket,
            head,
            (ws) => {
              websocketEndpoints[pathname].emit("connection", ws, request);
            },
          );
        } else {
          socket.destroy();
        }
      });
      if (onReady) {
        onReady(httpServer);
      }
      resolve({
        httpServer,
      });
    });
    httpServer.timeout = 0;
    httpServer.on("close", () => {
      endMiddleware()?.finally(() => {
        onClose?.();
      });
    });
  });
};
exports.runServer = runServer;
const runBuild = async (
  config,
  {
    assets = false,
    customResolverOptions = DEFAULTS.customResolverOptions,
    customTransformOptions = DEFAULTS.customTransformOptions,
    dev = false,
    entry,
    onBegin,
    onComplete,
    onProgress,
    minify = true,
    output = outputBundle,
    out,
    bundleOut,
    sourceMapOut,
    platform = "web",
    sourceMap = false,
    sourceMapUrl,
    unstable_transformProfile = DEFAULTS.unstable_transformProfile,
  },
) => {
  const metroServer = await runMetro(config, {
    watch: false,
  });
  try {
    const requestOptions = {
      dev,
      entryFile: entry,
      inlineSourceMap: sourceMap && !sourceMapUrl,
      minify,
      platform,
      ...(sourceMap === false
        ? {}
        : {
            sourceMapUrl,
          }),
      createModuleIdFactory: config.serializer.createModuleIdFactory,
      onProgress,
      customResolverOptions,
      customTransformOptions,
      unstable_transformProfile,
    };
    if (onBegin) {
      onBegin();
    }
    const metroBundle = await output.build(metroServer, requestOptions, {
      withAssets: assets,
    });
    const result = {
      ...metroBundle,
    };
    if (assets && result.assets == null) {
      result.assets = await metroServer.getAssets({
        ..._Server.default.DEFAULT_BUNDLE_OPTIONS,
        ...requestOptions,
      });
    }
    if (onComplete) {
      onComplete();
    }
    if (out || bundleOut) {
      const bundleOutput =
        bundleOut ?? (0, _nullthrows.default)(out).replace(/(\.js)?$/, ".js");
      const sourcemapOutput =
        sourceMap === false
          ? undefined
          : (sourceMapOut ?? out?.replace(/(\.js)?$/, ".map"));
      const outputOptions = {
        bundleOutput,
        sourcemapOutput,
        dev,
        platform,
      };
      await output.save(metroBundle, outputOptions, (message) =>
        config.reporter.update({
          type: "bundle_save_log",
          message,
        }),
      );
    }
    return result;
  } finally {
    await metroServer.end();
  }
};
exports.runBuild = runBuild;
const buildGraph = async function (
  config,
  {
    customTransformOptions = Object.create(null),
    dev = false,
    entries,
    minify = false,
    onProgress,
    platform = "web",
    type = "module",
  },
) {
  const mergedConfig = await getConfig(config);
  const bundler = new _IncrementalBundler.default(mergedConfig);
  try {
    const { customResolverOptions, ...defaultTransformInputOptions } =
      _Server.default.DEFAULT_GRAPH_OPTIONS;
    return await bundler.buildGraphForEntries(
      entries,
      {
        ...defaultTransformInputOptions,
        customTransformOptions,
        dev,
        minify,
        platform,
        type,
      },
      {
        customResolverOptions,
        dev,
      },
    );
  } finally {
    await bundler.end();
  }
};
exports.buildGraph = buildGraph;
const attachMetroCli = function (yargs, options = {}) {
  const { build = {}, serve = {}, dependencies = {} } = options;
  yargs.strict();
  if (build) {
    yargs.command((0, _build.default)());
  }
  if (serve) {
    yargs.command((0, _serve.default)());
  }
  if (dependencies) {
    yargs.command((0, _dependencies.default)());
  }
  return yargs;
};
exports.attachMetroCli = attachMetroCli;
async function earlyPortCheck(host, port) {
  const server = _net.default.createServer((c) => c.end());
  try {
    await new Promise((resolve, reject) => {
      server.on("error", (err) => {
        reject(err);
      });
      server.listen(port, host, undefined, () => resolve());
    });
  } finally {
    await new Promise((resolve) => server.close(() => resolve()));
  }
}
var _default = (exports.default = {
  attachMetroCli,
  runServer,
  Terminal: _metroCore.Terminal,
  JsonReporter: _JsonReporter.default,
  TerminalReporter: _TerminalReporter.default,
  loadConfig: _metroConfig.loadConfig,
  mergeConfig: _metroConfig.mergeConfig,
  resolveConfig: _metroConfig.resolveConfig,
  createConnectMiddleware,
  runBuild,
  buildGraph,
});
