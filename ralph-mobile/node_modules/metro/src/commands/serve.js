"use strict";

const { makeAsyncCommand, watchFile } = require("../cli-utils");
const { loadConfig, resolveConfig } = require("metro-config");
const { promisify } = require("util");
module.exports = () => ({
  command: "serve",
  aliases: ["start"],
  desc: "Starts Metro on the given port, building bundles on the fly",
  builder: (yargs) => {
    yargs.option("project-roots", {
      alias: "P",
      type: "string",
      array: true,
    });
    yargs.option("host", {
      alias: "h",
      type: "string",
      default: "localhost",
    });
    yargs.option("port", {
      alias: "p",
      type: "number",
      default: 8081,
    });
    yargs.option("max-workers", {
      alias: "j",
      type: "number",
    });
    yargs.option("secure", {
      type: "boolean",
      describe: "(deprecated)",
    });
    yargs.option("secure-key", {
      type: "string",
      describe: "(deprecated)",
    });
    yargs.option("secure-cert", {
      type: "string",
      describe: "(deprecated)",
    });
    yargs.option("secure-server-options", {
      alias: "s",
      type: "string",
      describe: "Use dot notation for object path",
    });
    yargs.option("hmr-enabled", {
      alias: "hmr",
      type: "boolean",
    });
    yargs.option("config", {
      alias: "c",
      type: "string",
    });
    yargs.option("reset-cache", {
      type: "boolean",
    });
    yargs.example(
      "secure-server-options",
      '-s.cert="$(cat path/to/cert)" -s.key="$(cat path/to/key")'
    );
  },
  handler: makeAsyncCommand(async (argv) => {
    let server = null;
    let restarting = false;
    async function restart() {
      if (restarting) {
        return;
      } else {
        restarting = true;
      }
      if (server) {
        console.log("Configuration changed. Restarting the server...");
        await promisify(server.close).call(server);
      }
      const config = await loadConfig(argv);
      const MetroApi = require("../index");
      const {
        config: _config,
        hmrEnabled: _hmrEnabled,
        maxWorkers: _maxWorkers,
        port: _port,
        projectRoots: _projectRoots,
        resetCache: _resetCache,
        ...runServerOptions
      } = argv;
      server = await MetroApi.runServer(config, runServerOptions);
      restarting = false;
    }
    const foundConfig = await resolveConfig(argv.config);
    if (foundConfig) {
      await watchFile(foundConfig.filepath, restart);
    } else {
      await restart();
    }
  }),
});
