"use strict";

var _parseKeyValueParamArray = _interopRequireDefault(
  require("../cli/parseKeyValueParamArray")
);
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const { makeAsyncCommand } = require("../cli-utils");
const TerminalReporter = require("../lib/TerminalReporter");
const { loadConfig } = require("metro-config");
const { Terminal } = require("metro-core");
const term = new Terminal(process.stdout);
const updateReporter = new TerminalReporter(term);
module.exports = () => ({
  command: "build <entry>",
  desc: "Generates a JavaScript bundle containing the specified entrypoint and its descendants",
  builder: (yargs) => {
    yargs.option("project-roots", {
      alias: "P",
      type: "string",
      array: true,
    });
    yargs.option("out", {
      alias: "O",
      type: "string",
      demandOption: true,
    });
    yargs.option("platform", {
      alias: "p",
      type: "string",
    });
    yargs.option("output-type", {
      alias: "t",
      type: "string",
    });
    yargs.option("max-workers", {
      alias: "j",
      type: "number",
    });
    yargs.option("minify", {
      alias: "z",
      type: "boolean",
    });
    yargs.option("dev", {
      alias: "g",
      type: "boolean",
    });
    yargs.option("source-map", {
      type: "boolean",
    });
    yargs.option("source-map-url", {
      type: "string",
    });
    yargs.option("legacy-bundler", {
      type: "boolean",
    });
    yargs.option("config", {
      alias: "c",
      type: "string",
    });
    yargs.option("transform-option", {
      type: "string",
      array: true,
      alias: "transformer-option",
      coerce: _parseKeyValueParamArray.default,
      describe:
        "Custom transform options of the form key=value. URL-encoded. May be specified multiple times.",
    });
    yargs.option("resolver-option", {
      type: "string",
      array: true,
      coerce: _parseKeyValueParamArray.default,
      describe:
        "Custom resolver options of the form key=value. URL-encoded. May be specified multiple times.",
    });
    yargs.option("reset-cache", {
      type: "boolean",
    });
  },
  handler: makeAsyncCommand(async (argv) => {
    const config = await loadConfig(argv);
    const options = {
      entry: argv.entry,
      dev: argv.dev,
      out: argv.out,
      minify: argv.minify,
      platform: argv.platform,
      sourceMap: argv.sourceMap,
      sourceMapUrl: argv.sourceMapUrl,
      customResolverOptions: argv.resolverOption,
      customTransformOptions: argv.transformOption,
    };
    const MetroApi = require("../index");
    await MetroApi.runBuild(config, {
      ...options,
      onBegin: () => {
        updateReporter.update({
          buildID: "$",
          type: "bundle_build_started",
          bundleDetails: {
            bundleType: "Bundle",
            customResolverOptions: options.customResolverOptions ?? {},
            customTransformOptions: options.customTransformOptions ?? {},
            dev: !!options.dev,
            entryFile: options.entry,
            minify: !!options.minify,
            platform: options.platform,
          },
        });
      },
      onProgress: (transformedFileCount, totalFileCount) => {
        updateReporter.update({
          buildID: "$",
          type: "bundle_transform_progressed",
          transformedFileCount,
          totalFileCount,
        });
      },
      onComplete: () => {
        updateReporter.update({
          buildID: "$",
          type: "bundle_build_done",
        });
      },
    });
  }),
});
