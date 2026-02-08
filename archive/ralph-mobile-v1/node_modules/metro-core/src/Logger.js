"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.createActionEndEntry = createActionEndEntry;
exports.createActionStartEntry = createActionStartEntry;
exports.createEntry = createEntry;
exports.log = log;
exports.on = on;
var _events = _interopRequireDefault(require("events"));
var _os = _interopRequireDefault(require("os"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const VERSION = require("../package.json").version;
const log_session = `${_os.default.hostname()}-${Date.now()}`;
const eventEmitter = new _events.default();
function on(event, handler) {
  eventEmitter.on(event, handler);
}
function createEntry(data) {
  const logEntry =
    typeof data === "string"
      ? {
          log_entry_label: data,
        }
      : data;
  return {
    ...logEntry,
    log_session,
    metro_bundler_version: VERSION,
  };
}
function createActionStartEntry(data) {
  const logEntry =
    typeof data === "string"
      ? {
          action_name: data,
        }
      : data;
  const { action_name } = logEntry;
  return createEntry({
    log_entry_label: action_name,
    ...logEntry,
    action_name,
    action_phase: "start",
    start_timestamp: process.hrtime(),
  });
}
function createActionEndEntry(logEntry, error) {
  const { action_name, action_phase, start_timestamp } = logEntry;
  if (action_phase !== "start" || !Array.isArray(start_timestamp)) {
    throw new Error("Action has not started or has already ended");
  }
  const timeDelta = process.hrtime(start_timestamp);
  const duration_ms = Math.round((timeDelta[0] * 1e9 + timeDelta[1]) / 1e6);
  return createEntry({
    log_entry_label: action_name,
    ...logEntry,
    action_name,
    action_phase: "end",
    duration_ms,
    ...(error != null
      ? {
          error_message: error.message,
          error_stack: error.stack,
        }
      : null),
  });
}
function log(logEntry) {
  eventEmitter.emit("log", logEntry);
  return logEntry;
}
