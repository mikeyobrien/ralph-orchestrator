"use strict";

const terser = require("terser");
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
  const result = await terser.minify(code, options);
  return {
    code: result.code,
    map: result.map,
  };
}
module.exports = minifier;
