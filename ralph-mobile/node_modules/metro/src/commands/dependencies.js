"use strict";

const { makeAsyncCommand } = require("../cli-utils");
const Server = require("../Server");
const fs = require("fs");
const { loadConfig } = require("metro-config");
const path = require("path");
const { promisify } = require("util");
async function dependencies(args, config) {
  const rootModuleAbsolutePath = args.entryFile;
  if (!fs.existsSync(rootModuleAbsolutePath)) {
    return Promise.reject(
      new Error(`File ${rootModuleAbsolutePath} does not exist`)
    );
  }
  config.cacheStores = [];
  const relativePath = path.relative(
    config.server.unstable_serverRoot ?? config.projectRoot,
    rootModuleAbsolutePath
  );
  const options = {
    platform: args.platform,
    entryFile: relativePath,
    dev: args.dev,
    minify: false,
    generateSourceMaps: !args.dev,
  };
  const outStream =
    args.output != null ? fs.createWriteStream(args.output) : process.stdout;
  const server = new Server(config);
  try {
    const deps = await server.getOrderedDependencyPaths(options);
    deps.forEach((modulePath) => {
      const isInsideProjectRoots =
        config.watchFolders.filter((root) => modulePath.startsWith(root))
          .length > 0;
      if (isInsideProjectRoots) {
        outStream.write(modulePath + "\n");
      }
    });
  } finally {
    await server.end();
  }
  return args.output != null
    ? promisify(outStream.end).bind(outStream)()
    : Promise.resolve();
}
module.exports = () => ({
  command: "get-dependencies [entryFile]",
  desc: "List all dependencies that will be bundled for a given entry point",
  builder: (yargs) => {
    yargs.option("entry-file", {
      type: "string",
      demandOption: true,
      describe: "Absolute path to the root JS file",
    });
    yargs.option("output", {
      type: "string",
      describe:
        "File name where to store the output, ex. /tmp/dependencies.txt",
    });
    yargs.option("platform", {
      type: "string",
      describe: "The platform extension used for selecting modules",
    });
    yargs.option("transformer", {
      type: "string",
      describe: "Specify a custom transformer to be used",
    });
    yargs.option("max-workers", {
      type: "number",
      describe:
        "Specifies the maximum number of workers the worker-pool " +
        "will spawn for transforming files. This defaults to the number of the " +
        "cores available on your machine.",
    });
    yargs.option("dev", {
      type: "boolean",
      default: true,
      describe: "If false, skip all dev-only code path",
    });
    yargs.option("verbose", {
      type: "boolean",
      default: false,
      description: "Enables logging",
    });
  },
  handler: makeAsyncCommand(async (argv) => {
    const config = await loadConfig(argv);
    await dependencies(argv, config);
  }),
});
