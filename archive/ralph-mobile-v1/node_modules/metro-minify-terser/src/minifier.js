"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = minifier;
var _terser = _interopRequireDefault(require("terser"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
async function minifier(options) {
  const result = await minify(options);
  if (!options.map || result.map == null) {
    return {
      code: result.code,
    };
  }
  const map = JSON.parse(result.map);
  return {
    code: result.code,
    map: {
      ...map,
      sources: [options.filename],
    },
  };
}
async function minify({ code, map, reserved, config }) {
  const options = {
    ...config,
    output: {
      ...(config.output ?? {}),
    },
    mangle:
      config.mangle === false
        ? false
        : {
            ...config.mangle,
            reserved,
          },
    sourceMap: map
      ? config.sourceMap === false
        ? false
        : {
            ...config.sourceMap,
            content: map,
          }
      : false,
  };
  const result = await _terser.default.minify(code, options);
  return {
    code: result.code,
    map: result.map,
  };
}
