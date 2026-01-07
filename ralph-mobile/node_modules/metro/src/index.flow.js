"use strict";

const makeBuildCommand = require("./commands/build");
const makeDependenciesCommand = require("./commands/dependencies");
const makeServeCommand = require("./commands/serve");
const MetroHmrServer = require("./HmrServer");
const IncrementalBundler = require("./IncrementalBundler");
const createWebsocketServer = require("./lib/createWebsocketServer");
const TerminalReporter = require("./lib/TerminalReporter");
const MetroServer = require("./Server");
const outputBundle = require("./shared/output/bundle");
const chalk = require("chalk");
const fs = require("fs");
const http = require("http");
const https = require("https");
const {
  getDefaultConfig,
  loadConfig,
  mergeConfig,
  resolveConfig,
} = require("metro-config");
const { Terminal } = require("metro-core");
const net = require("net");
const { parse } = require("url");
exports.Terminal = Terminal;
exports.TerminalReporter = TerminalReporter;
async function getConfig(config) {
  const defaultConfig = await getDefaultConfig(config.projectRoot);
  return mergeConfig(defaultConfig, config);
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
  const server = new MetroServer(mergedConfig, serverOptions);
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
exports.runMetro = runMetro;
exports.loadConfig = loadConfig;
exports.mergeConfig = mergeConfig;
exports.resolveConfig = resolveConfig;
const createConnectMiddleware = async function (config, options) {
  const metroServer = await runMetro(config, options);
  let enhancedMiddleware = metroServer.processRequest;
  if (config.server.enhanceMiddleware) {
    enhancedMiddleware = config.server.enhanceMiddleware(
      enhancedMiddleware,
      metroServer
    );
  }
  return {
    attachHmrServer(httpServer) {
      const wss = createWebsocketServer({
        websocketServer: new MetroHmrServer(
          metroServer.getBundler(),
          metroServer.getCreateModuleId(),
          config
        ),
      });
      httpServer.on("upgrade", (request, socket, head) => {
        const { pathname } = parse(request.url);
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
exports.runServer = async (
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
  } = {}
) => {
  await earlyPortCheck(host, config.server.port);
  if (secure != null || secureCert != null || secureKey != null) {
    console.warn(
      chalk.inverse.yellow.bold(" DEPRECATED "),
      "The `secure`, `secureCert`, and `secureKey` options are now deprecated. " +
        "Please use the `secureServerOptions` object instead to pass options to " +
        "Metro's https development server."
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
        key: fs.readFileSync(secureKey),
        cert: fs.readFileSync(secureCert),
        ...secureServerOptions,
      };
    }
    httpServer = https.createServer(options, serverApp);
  } else {
    httpServer = http.createServer(serverApp);
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
        "/hot": createWebsocketServer({
          websocketServer: new MetroHmrServer(
            metroServer.getBundler(),
            metroServer.getCreateModuleId(),
            config
          ),
        }),
      };
      httpServer.on("upgrade", (request, socket, head) => {
        const { pathname } = parse(request.url);
        if (pathname != null && websocketEndpoints[pathname]) {
          websocketEndpoints[pathname].handleUpgrade(
            request,
            socket,
            head,
            (ws) => {
              websocketEndpoints[pathname].emit("connection", ws, request);
            }
          );
        } else {
          socket.destroy();
        }
      });
      if (onReady) {
        onReady(httpServer);
      }
      resolve(httpServer);
    });
    httpServer.timeout = 0;
    httpServer.on("close", () => {
      endMiddleware()?.finally(() => {
        onClose?.();
      });
    });
  });
};
exports.runBuild = async (
  config,
  {
    customResolverOptions,
    customTransformOptions,
    dev = false,
    entry,
    onBegin,
    onComplete,
    onProgress,
    minify = true,
    output = outputBundle,
    out,
    platform = "web",
    sourceMap = false,
    sourceMapUrl,
  }
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
      sourceMapUrl: sourceMap === false ? undefined : sourceMapUrl,
      createModuleIdFactory: config.serializer.createModuleIdFactory,
      onProgress,
      customResolverOptions,
      customTransformOptions,
    };
    if (onBegin) {
      onBegin();
    }
    const metroBundle = await output.build(metroServer, requestOptions);
    if (onComplete) {
      onComplete();
    }
    if (out) {
      const bundleOutput = out.replace(/(\.js)?$/, ".js");
      const sourcemapOutput =
        sourceMap === false ? undefined : out.replace(/(\.js)?$/, ".map");
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
        })
      );
    }
    return metroBundle;
  } finally {
    await metroServer.end();
  }
};
exports.buildGraph = async function (
  config,
  {
    customTransformOptions = Object.create(null),
    dev = false,
    entries,
    minify = false,
    onProgress,
    platform = "web",
    type = "module",
  }
) {
  const mergedConfig = await getConfig(config);
  const bundler = new IncrementalBundler(mergedConfig);
  try {
    const { customResolverOptions, ...defaultTransformInputOptions } =
      MetroServer.DEFAULT_GRAPH_OPTIONS;
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
      }
    );
  } finally {
    await bundler.end();
  }
};
exports.attachMetroCli = function (yargs, options = {}) {
  const { build = {}, serve = {}, dependencies = {} } = options;
  yargs.strict();
  if (build) {
    yargs.command(makeBuildCommand());
  }
  if (serve) {
    yargs.command(makeServeCommand());
  }
  if (dependencies) {
    yargs.command(makeDependenciesCommand());
  }
  return yargs;
};
async function earlyPortCheck(host, port) {
  const server = net.createServer((c) => c.end());
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
