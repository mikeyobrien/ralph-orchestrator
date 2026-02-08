"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _formatFileCandidates = _interopRequireDefault(
  require("./formatFileCandidates"),
);
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class InvalidPackageError extends Error {
  constructor(opts) {
    super(
      `The package \`${opts.packageJsonPath}\` is invalid because it ` +
        "specifies a `main` module field that could not be resolved (" +
        `\`${opts.mainModulePath}\`. None of these files exist:\n\n` +
        `  * ${(0, _formatFileCandidates.default)(opts.fileCandidates)}\n` +
        `  * ${(0, _formatFileCandidates.default)(opts.indexCandidates)}`,
    );
    Object.assign(this, opts);
  }
}
exports.default = InvalidPackageError;
