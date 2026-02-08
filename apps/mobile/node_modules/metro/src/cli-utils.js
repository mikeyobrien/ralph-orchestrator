"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.watchFile = exports.makeAsyncCommand = void 0;
var _fs = _interopRequireDefault(require("fs"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const watchFile = async function (filename, callback) {
  _fs.default.watchFile(filename, () => {
    callback();
  });
  await callback();
};
exports.watchFile = watchFile;
const makeAsyncCommand = (command) => (argv) => {
  Promise.resolve(command(argv)).catch((error) => {
    console.error(error.stack);
    process.exitCode = 1;
  });
};
exports.makeAsyncCommand = makeAsyncCommand;
