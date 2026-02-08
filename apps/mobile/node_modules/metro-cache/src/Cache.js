"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _metroCore = require("metro-core");
class Cache {
  #stores;
  #hits = new WeakMap();
  constructor(stores) {
    this.#stores = stores;
  }
  async get(key) {
    const stores = this.#stores;
    const length = stores.length;
    for (let i = 0; i < length; i++) {
      const store = stores[i];
      const storeName = store.name ?? store.constructor.name;
      const name = storeName + "::" + key.toString("hex");
      let value = null;
      const logStart = _metroCore.Logger.log(
        _metroCore.Logger.createActionStartEntry({
          action_name: "Cache get",
          log_entry_label: name,
        }),
      );
      try {
        const valueOrPromise = store.get(key);
        if (valueOrPromise && typeof valueOrPromise.then === "function") {
          value = await valueOrPromise;
        } else {
          value = valueOrPromise;
        }
      } finally {
        const hitOrMiss = value != null ? "hit" : "miss";
        _metroCore.Logger.log({
          ..._metroCore.Logger.createActionEndEntry(logStart),
          action_result: hitOrMiss,
        });
        _metroCore.Logger.log(
          _metroCore.Logger.createEntry({
            action_name: "Cache " + hitOrMiss,
            log_entry_label: name,
          }),
        );
        if (value != null) {
          this.#hits.set(key, store);
          return value;
        }
      }
    }
    return null;
  }
  async set(key, value) {
    const stores = this.#stores;
    const stop = this.#hits.get(key);
    const length = stores.length;
    const promises = [];
    const writeErrors = [];
    const storesWithErrors = new Set();
    for (let i = 0; i < length && stores[i] !== stop; i++) {
      const store = stores[i];
      const storeName = store.name ?? store.constructor.name;
      const name = storeName + "::" + key.toString("hex");
      const logStart = _metroCore.Logger.log(
        _metroCore.Logger.createActionStartEntry({
          action_name: "Cache set",
          log_entry_label: name,
        }),
      );
      promises.push(
        (async () => {
          try {
            await stores[i].set(key, value);
            _metroCore.Logger.log(
              _metroCore.Logger.createActionEndEntry(logStart),
            );
          } catch (e) {
            _metroCore.Logger.log(
              _metroCore.Logger.createActionEndEntry(logStart, e),
            );
            storesWithErrors.add(storeName);
            writeErrors.push(
              new Error(`Cache write failed for ${name}`, {
                cause: e,
              }),
            );
          }
        })(),
      );
    }
    await Promise.allSettled(promises);
    if (writeErrors.length > 0) {
      throw new AggregateError(
        writeErrors,
        `Cache write failed for store(s): ${Array.from(storesWithErrors).join(", ")}`,
      );
    }
  }
  get isDisabled() {
    return this.#stores.length === 0;
  }
}
exports.default = Cache;
