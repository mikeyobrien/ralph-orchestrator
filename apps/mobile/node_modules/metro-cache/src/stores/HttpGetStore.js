"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _HttpStore = _interopRequireDefault(require("./HttpStore"));
var _metroCore = require("metro-core");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class HttpGetStore extends _HttpStore.default {
  #warned;
  constructor(options) {
    super(options);
    this.#warned = false;
  }
  async get(key) {
    try {
      return await super.get(key);
    } catch (err) {
      if (
        !(err instanceof _HttpStore.default.HttpError) &&
        !(err instanceof _HttpStore.default.NetworkError)
      ) {
        throw err;
      }
      this.#warn(err);
      return null;
    }
  }
  async set(_key, _value) {}
  #warn(err) {
    if (!this.#warned) {
      process.emitWarning(
        [
          "Could not connect to the HTTP cache.",
          "Original error: " + err.message,
        ].join(" "),
      );
      _metroCore.Logger.log(
        _metroCore.Logger.createEntry({
          action_name: "HttpGetStore:Warning",
          log_entry_label: `${err.message} (${err.code})`,
        }),
      );
      this.#warned = true;
    }
  }
}
exports.default = HttpGetStore;
