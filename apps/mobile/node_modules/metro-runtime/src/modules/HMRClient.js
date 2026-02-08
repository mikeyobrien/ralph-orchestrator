"use strict";

const EventEmitter = require("./vendor/eventemitter3");
const inject = ({ module: [id, code], sourceURL }) => {
  if (global.globalEvalWithSourceUrl) {
    global.globalEvalWithSourceUrl(code, sourceURL);
  } else {
    eval(code);
  }
};
const injectUpdate = (update) => {
  update.added.forEach(inject);
  update.modified.forEach(inject);
};
class HMRClient extends EventEmitter {
  _isEnabled = false;
  _pendingUpdate = null;
  _queue = [];
  _state = "opening";
  constructor(url) {
    super();
    this._ws = new global.WebSocket(url);
    this._ws.onopen = () => {
      this._state = "open";
      this.emit("open");
      this._flushQueue();
    };
    this._ws.onerror = (error) => {
      this.emit("connection-error", error);
    };
    this._ws.onclose = (closeEvent) => {
      this._state = "closed";
      this.emit("close", closeEvent);
    };
    this._ws.onmessage = (message) => {
      const data = JSON.parse(String(message.data));
      switch (data.type) {
        case "bundle-registered":
          this.emit("bundle-registered");
          break;
        case "update-start":
          this.emit("update-start", data.body);
          break;
        case "update":
          this.emit("update", data.body);
          break;
        case "update-done":
          this.emit("update-done");
          break;
        case "error":
          this.emit("error", data.body);
          break;
        default:
          this.emit("error", {
            type: "unknown-message",
            message: data,
          });
      }
    };
    this.on("update", (update) => {
      if (this._isEnabled) {
        injectUpdate(update);
      } else if (this._pendingUpdate == null) {
        this._pendingUpdate = update;
      } else {
        this._pendingUpdate = mergeUpdates(this._pendingUpdate, update);
      }
    });
  }
  close() {
    this._ws.close();
  }
  send(message) {
    switch (this._state) {
      case "opening":
        this._queue.push(message);
        break;
      case "open":
        this._ws.send(message);
        break;
      case "closed":
        break;
      default:
        throw new Error("[WebSocketHMRClient] Unknown state: " + this._state);
    }
  }
  _flushQueue() {
    this._queue.forEach((message) => this.send(message));
    this._queue.length = 0;
  }
  enable() {
    this._isEnabled = true;
    const update = this._pendingUpdate;
    this._pendingUpdate = null;
    if (update != null) {
      injectUpdate(update);
    }
  }
  disable() {
    this._isEnabled = false;
  }
  isEnabled() {
    return this._isEnabled;
  }
  hasPendingUpdates() {
    return this._pendingUpdate != null;
  }
}
function mergeUpdates(base, next) {
  const addedIDs = new Set();
  const deletedIDs = new Set();
  const moduleMap = new Map();
  applyUpdateLocally(base);
  applyUpdateLocally(next);
  function applyUpdateLocally(update) {
    update.deleted.forEach((id) => {
      if (addedIDs.has(id)) {
        addedIDs.delete(id);
      } else {
        deletedIDs.add(id);
      }
      moduleMap.delete(id);
    });
    update.added.forEach((item) => {
      const id = item.module[0];
      if (deletedIDs.has(id)) {
        deletedIDs.delete(id);
      } else {
        addedIDs.add(id);
      }
      moduleMap.set(id, item);
    });
    update.modified.forEach((item) => {
      const id = item.module[0];
      moduleMap.set(id, item);
    });
  }
  const result = {
    isInitialUpdate: next.isInitialUpdate,
    revisionId: next.revisionId,
    added: [],
    modified: [],
    deleted: [],
  };
  deletedIDs.forEach((id) => {
    result.deleted.push(id);
  });
  moduleMap.forEach((item, id) => {
    if (deletedIDs.has(id)) {
      return;
    }
    if (addedIDs.has(id)) {
      result.added.push(item);
    } else {
      result.modified.push(item);
    }
  });
  return result;
}
module.exports = HMRClient;
