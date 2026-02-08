"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = getMaxWorkers;
var _os = _interopRequireDefault(require("os"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function getMaxWorkers(workers) {
  const cores = _os.default.availableParallelism();
  return typeof workers === "number" && Number.isInteger(workers)
    ? Math.min(cores, workers > 0 ? workers : 1)
    : Math.max(1, Math.ceil(cores * (0.5 + 0.5 * Math.exp(-cores * 0.07)) - 1));
}
