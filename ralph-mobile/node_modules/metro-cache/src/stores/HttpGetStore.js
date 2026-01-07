"use strict";

const HttpStore = require("./HttpStore");
const { Logger } = require("metro-core");
class HttpGetStore extends HttpStore {
  constructor(options) {
    super(options);
    this._warned = false;
  }
  async get(key) {
    try {
      return await super.get(key);
    } catch (err) {
      if (
        !(err instanceof HttpStore.HttpError) &&
        !(err instanceof HttpStore.NetworkError)
      ) {
        throw err;
      }
      this._warn(err);
      return null;
    }
  }
  set() {
    return Promise.resolve(undefined);
  }
  _warn(err) {
    if (!this._warned) {
      process.emitWarning(
        [
          "Could not connect to the HTTP cache.",
          "Original error: " + err.message,
        ].join(" ")
      );
      Logger.log(
        Logger.createEntry({
          action_name: "HttpGetStore:Warning",
          log_entry_label: `${err.message} (${err.code})`,
        })
      );
      this._warned = true;
    }
  }
}
module.exports = HttpGetStore;
