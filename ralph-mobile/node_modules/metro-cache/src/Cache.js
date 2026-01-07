"use strict";

const { Logger } = require("metro-core");
class Cache {
  constructor(stores) {
    this._hits = new WeakMap();
    this._stores = stores;
  }
  async get(key) {
    const stores = this._stores;
    const length = stores.length;
    for (let i = 0; i < length; i++) {
      const store = stores[i];
      const storeName = store.name ?? store.constructor.name;
      const name = storeName + "::" + key.toString("hex");
      let value = null;
      const logStart = Logger.log(
        Logger.createActionStartEntry({
          action_name: "Cache get",
          log_entry_label: name,
        })
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
        Logger.log({
          ...Logger.createActionEndEntry(logStart),
          action_result: hitOrMiss,
        });
        Logger.log(
          Logger.createEntry({
            action_name: "Cache " + hitOrMiss,
            log_entry_label: name,
          })
        );
        if (value != null) {
          this._hits.set(key, store);
          return value;
        }
      }
    }
    return null;
  }
  async set(key, value) {
    const stores = this._stores;
    const stop = this._hits.get(key);
    const length = stores.length;
    const promises = [];
    const writeErrors = [];
    const storesWithErrors = new Set();
    for (let i = 0; i < length && stores[i] !== stop; i++) {
      const store = stores[i];
      const storeName = store.name ?? store.constructor.name;
      const name = storeName + "::" + key.toString("hex");
      const logStart = Logger.log(
        Logger.createActionStartEntry({
          action_name: "Cache set",
          log_entry_label: name,
        })
      );
      promises.push(
        (async () => {
          try {
            await stores[i].set(key, value);
            Logger.log(Logger.createActionEndEntry(logStart));
          } catch (e) {
            Logger.log(Logger.createActionEndEntry(logStart, e));
            storesWithErrors.add(storeName);
            writeErrors.push(
              new Error(`Cache write failed for ${name}`, {
                cause: e,
              })
            );
          }
        })()
      );
    }
    await Promise.allSettled(promises);
    if (writeErrors.length > 0) {
      throw new AggregateError(
        writeErrors,
        `Cache write failed for store(s): ${Array.from(storesWithErrors).join(
          ", "
        )}`
      );
    }
  }
  get isDisabled() {
    return this._stores.length === 0;
  }
}
module.exports = Cache;
