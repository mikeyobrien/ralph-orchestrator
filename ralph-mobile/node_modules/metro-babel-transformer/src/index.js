"use strict";

const { parseSync, transformFromAstSync } = require("@babel/core");
const nullthrows = require("nullthrows");
function transform({ filename, options, plugins, src }) {
  const OLD_BABEL_ENV = process.env.BABEL_ENV;
  process.env.BABEL_ENV = options.dev
    ? "development"
    : process.env.BABEL_ENV || "production";
  try {
    const babelConfig = {
      caller: {
        name: "metro",
        bundler: "metro",
        platform: options.platform,
      },
      ast: true,
      babelrc: options.enableBabelRCLookup,
      code: false,
      cwd: options.projectRoot,
      highlightCode: true,
      filename,
      plugins,
      sourceType: "module",
      cloneInputAst: false,
    };
    const sourceAst = options.hermesParser
      ? require("hermes-parser").parse(src, {
          babel: true,
          sourceType: babelConfig.sourceType,
        })
      : parseSync(src, babelConfig);
    const transformResult = transformFromAstSync(sourceAst, src, babelConfig);
    return {
      ast: nullthrows(transformResult.ast),
      metadata: transformResult.metadata,
    };
  } finally {
    if (OLD_BABEL_ENV) {
      process.env.BABEL_ENV = OLD_BABEL_ENV;
    }
  }
}
module.exports = {
  transform,
};
