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
class FailedToResolvePathError extends Error {
  constructor(candidates) {
    super(
      "The module could not be resolved because none of these files exist:\n\n" +
        [candidates.file, candidates.dir]
          .filter(Boolean)
          .map(
            (candidates) =>
              `  * ${(0, _formatFileCandidates.default)(candidates)}`,
          )
          .join("\n"),
    );
    this.candidates = candidates;
  }
}
exports.default = FailedToResolvePathError;
