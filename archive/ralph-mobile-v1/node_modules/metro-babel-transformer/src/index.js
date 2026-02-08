"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
exports.transform = transform;
var _core = require("@babel/core");
var _nullthrows = _interopRequireDefault(require("nullthrows"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
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
      : (0, _core.parseSync)(src, babelConfig);
    const transformResult = (0, _core.transformFromAstSync)(
      sourceAst,
      src,
      babelConfig,
    );
    return {
      ast: (0, _nullthrows.default)(transformResult.ast),
      metadata: transformResult.metadata,
    };
  } finally {
    if (OLD_BABEL_ENV) {
      process.env.BABEL_ENV = OLD_BABEL_ENV;
    }
  }
}
var _default = (exports.default = {
  transform,
});
