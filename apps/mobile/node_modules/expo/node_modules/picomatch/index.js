'use strict';

const os = require('os');
const pico = require('./lib/picomatch');

const isWindows = os.platform() === 'win32';

function picomatch(glob, options, returnState = false) {
  // default to os.platform()
  if (options && (options.windows === null || options.windows === undefined)) {
    // don't mutate the original options object
    options = { ...options, windows: isWindows };
  }
  return pico(glob, options, returnState);
}

module.exports = picomatch;
// public api
module.exports.test = pico.test;
module.exports.matchBase = pico.matchBase;
module.exports.isMatch = pico.isMatch;
module.exports.parse = pico.parse;
module.exports.scan = pico.scan;
module.exports.compileRe = pico.compileRe;
module.exports.toRegex = pico.toRegex;
// for tests
module.exports.makeRe = pico.makeRe;
