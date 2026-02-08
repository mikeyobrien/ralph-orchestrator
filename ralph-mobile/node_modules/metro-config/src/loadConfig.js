"use strict";

const getDefaultConfig = require("./defaults");
const validConfig = require("./defaults/validConfig");
const cosmiconfig = require("cosmiconfig");
const fs = require("fs");
const { validate } = require("jest-validate");
const MetroCache = require("metro-cache");
const path = require("path");
const { dirname, join } = require("path");
function overrideArgument(arg) {
  if (arg == null) {
    return arg;
  }
  if (Array.isArray(arg)) {
    return arg[arg.length - 1];
  }
  return arg;
}
const explorer = cosmiconfig("metro", {
  searchPlaces: [
    "metro.config.js",
    "metro.config.cjs",
    "metro.config.json",
    "package.json",
  ],
  loaders: {
    ".json": cosmiconfig.loadJson,
    ".yaml": cosmiconfig.loadYaml,
    ".yml": cosmiconfig.loadYaml,
    ".js": cosmiconfig.loadJs,
    ".cjs": cosmiconfig.loadJs,
    ".es6": cosmiconfig.loadJs,
    noExt: cosmiconfig.loadYaml,
  },
});
const isFile = (filePath) =>
  fs.existsSync(filePath) && !fs.lstatSync(filePath).isDirectory();
const resolve = (filePath) => {
  try {
    return require.resolve(filePath);
  } catch (error) {
    if (path.isAbsolute(filePath) || error.code !== "MODULE_NOT_FOUND") {
      throw error;
    }
  }
  const possiblePath = path.resolve(process.cwd(), filePath);
  return isFile(possiblePath) ? possiblePath : filePath;
};
async function resolveConfig(filePath, cwd) {
  if (filePath) {
    return explorer.load(resolve(filePath));
  }
  const result = await explorer.search(cwd);
  if (result == null) {
    return {
      isEmpty: true,
      filepath: join(cwd || process.cwd(), "metro.config.stub.js"),
      config: {},
    };
  }
  return result;
}
function mergeConfig(defaultConfig, ...configs) {
  return configs.reduce(
    (totalConfig, nextConfig) => ({
      ...totalConfig,
      ...nextConfig,
      cacheStores:
        nextConfig.cacheStores != null
          ? typeof nextConfig.cacheStores === "function"
            ? nextConfig.cacheStores(MetroCache)
            : nextConfig.cacheStores
          : totalConfig.cacheStores,
      resolver: {
        ...totalConfig.resolver,
        ...(nextConfig.resolver || {}),
        dependencyExtractor:
          nextConfig.resolver && nextConfig.resolver.dependencyExtractor != null
            ? resolve(nextConfig.resolver.dependencyExtractor)
            : totalConfig.resolver.dependencyExtractor,
        hasteImplModulePath:
          nextConfig.resolver && nextConfig.resolver.hasteImplModulePath != null
            ? resolve(nextConfig.resolver.hasteImplModulePath)
            : totalConfig.resolver.hasteImplModulePath,
      },
      serializer: {
        ...totalConfig.serializer,
        ...(nextConfig.serializer || {}),
      },
      transformer: {
        ...totalConfig.transformer,
        ...(nextConfig.transformer || {}),
        babelTransformerPath:
          nextConfig.transformer &&
          nextConfig.transformer.babelTransformerPath != null
            ? resolve(nextConfig.transformer.babelTransformerPath)
            : totalConfig.transformer.babelTransformerPath,
      },
      server: {
        ...totalConfig.server,
        ...(nextConfig.server || {}),
      },
      symbolicator: {
        ...totalConfig.symbolicator,
        ...(nextConfig.symbolicator || {}),
      },
      watcher: {
        ...totalConfig.watcher,
        ...nextConfig.watcher,
        watchman: {
          ...totalConfig.watcher?.watchman,
          ...nextConfig.watcher?.watchman,
        },
        healthCheck: {
          ...totalConfig.watcher?.healthCheck,
          ...nextConfig.watcher?.healthCheck,
        },
        unstable_autoSaveCache: {
          ...totalConfig.watcher?.unstable_autoSaveCache,
          ...nextConfig.watcher?.unstable_autoSaveCache,
        },
      },
    }),
    defaultConfig
  );
}
async function loadMetroConfigFromDisk(path, cwd, defaultConfigOverrides) {
  const resolvedConfigResults = await resolveConfig(path, cwd);
  const { config: configModule, filepath } = resolvedConfigResults;
  const rootPath = dirname(filepath);
  const defaults = await getDefaultConfig(rootPath);
  const defaultConfig = mergeConfig(defaults, defaultConfigOverrides);
  if (typeof configModule === "function") {
    const resultedConfig = await configModule(defaultConfig);
    return mergeConfig(defaultConfig, resultedConfig);
  }
  return mergeConfig(defaultConfig, configModule);
}
function overrideConfigWithArguments(config, argv) {
  const output = {
    resolver: {},
    serializer: {},
    server: {},
    transformer: {},
  };
  if (argv.port != null) {
    output.server.port = Number(argv.port);
  }
  if (argv.projectRoot != null) {
    output.projectRoot = argv.projectRoot;
  }
  if (argv.watchFolders != null) {
    output.watchFolders = argv.watchFolders;
  }
  if (argv.assetExts != null) {
    output.resolver.assetExts = argv.assetExts;
  }
  if (argv.sourceExts != null) {
    output.resolver.sourceExts = argv.sourceExts;
  }
  if (argv.platforms != null) {
    output.resolver.platforms = argv.platforms;
  }
  if (argv["max-workers"] != null || argv.maxWorkers != null) {
    output.maxWorkers = Number(argv["max-workers"] || argv.maxWorkers);
  }
  if (argv.transformer != null) {
    output.transformer.babelTransformerPath = argv.transformer;
  }
  if (argv["reset-cache"] != null) {
    output.resetCache = argv["reset-cache"];
  }
  if (argv.resetCache != null) {
    output.resetCache = argv.resetCache;
  }
  if (argv.verbose === false) {
    output.reporter = {
      update: () => {},
    };
  }
  return mergeConfig(config, output);
}
async function loadConfig(argvInput = {}, defaultConfigOverrides = {}) {
  const argv = {
    ...argvInput,
    config: overrideArgument(argvInput.config),
  };
  const configuration = await loadMetroConfigFromDisk(
    argv.config,
    argv.cwd,
    defaultConfigOverrides
  );
  validate(configuration, {
    exampleConfig: await validConfig(),
    recursiveDenylist: ["reporter", "resolver", "transformer"],
    deprecatedConfig: {
      blacklistRE:
        () => `Warning: Metro config option \`blacklistRE\` is deprecated.
         Please use \`blockList\` instead.`,
    },
  });
  const configWithArgs = overrideConfigWithArguments(configuration, argv);
  return mergeConfig(configWithArgs, {
    watchFolders: [configWithArgs.projectRoot, ...configWithArgs.watchFolders],
  });
}
module.exports = {
  loadConfig,
  resolveConfig,
  mergeConfig,
};
