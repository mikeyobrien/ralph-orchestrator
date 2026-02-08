"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _lodash = _interopRequireDefault(require("lodash.throttle"));
var _readline = _interopRequireDefault(require("readline"));
var _tty = _interopRequireDefault(require("tty"));
var _util = _interopRequireDefault(require("util"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const { promisify } = _util.default;
const moveCursor = promisify(_readline.default.moveCursor);
const clearScreenDown = promisify(_readline.default.clearScreenDown);
const streamWrite = promisify((stream, chunk, callback) => {
  return stream.write(chunk, callback);
});
function chunkString(str, size) {
  const ANSI_COLOR = "\x1B\\[([0-9]{1,2}(;[0-9]{1,2})?)?m";
  const SKIP_ANSI = `(?:${ANSI_COLOR})*`;
  return str.match(new RegExp(`(?:${SKIP_ANSI}.){1,${size}}`, "g")) || [];
}
function getTTYStream(stream) {
  if (
    stream instanceof _tty.default.WriteStream &&
    stream.isTTY &&
    stream.columns >= 1
  ) {
    return stream;
  }
  return null;
}
class Terminal {
  #logLines;
  #nextStatusStr;
  #statusStr;
  #stream;
  #ttyStream;
  #updatePromise;
  #isUpdating;
  #isPendingUpdate;
  #shouldFlush;
  #writeStatusThrottled;
  constructor(stream, opts = {}) {
    this.#logLines = [];
    this.#nextStatusStr = "";
    this.#statusStr = "";
    this.#stream = stream;
    this.#ttyStream = (opts.ttyPrint ?? true) ? getTTYStream(stream) : null;
    this.#updatePromise = null;
    this.#isUpdating = false;
    this.#isPendingUpdate = false;
    this.#shouldFlush = false;
    this.#writeStatusThrottled = (0, _lodash.default)(
      (status) => this.#stream.write(status),
      3500,
    );
  }
  #scheduleUpdate() {
    if (this.#isUpdating) {
      this.#isPendingUpdate = true;
      return;
    }
    this.#isUpdating = true;
    this.#updatePromise = this.#update().then(async () => {
      while (this.#isPendingUpdate) {
        if (!this.#shouldFlush) {
          await new Promise((resolve) => setTimeout(resolve, 33));
        }
        this.#isPendingUpdate = false;
        await this.#update();
      }
      this.#isUpdating = false;
      this.#shouldFlush = false;
    });
  }
  async waitForUpdates() {
    await (this.#updatePromise || Promise.resolve());
  }
  async flush() {
    if (this.#isUpdating) {
      this.#shouldFlush = true;
    }
    await this.waitForUpdates();
    this.#writeStatusThrottled.flush();
  }
  async #update() {
    const ttyStream = this.#ttyStream;
    const nextStatusStr = this.#nextStatusStr;
    const statusStr = this.#statusStr;
    const logLines = this.#logLines;
    this.#statusStr = nextStatusStr;
    this.#logLines = [];
    if (statusStr === nextStatusStr && logLines.length === 0) {
      return;
    }
    if (ttyStream && statusStr.length > 0) {
      const statusLinesCount = statusStr.split("\n").length - 1;
      await moveCursor(ttyStream, -ttyStream.columns, -statusLinesCount - 1);
      await clearScreenDown(ttyStream);
    }
    if (logLines.length > 0) {
      await streamWrite(this.#stream, logLines.join("\n") + "\n");
    }
    if (ttyStream) {
      if (nextStatusStr.length > 0) {
        await streamWrite(this.#stream, nextStatusStr + "\n");
      }
    } else {
      this.#writeStatusThrottled(
        nextStatusStr.length > 0 ? nextStatusStr + "\n" : "",
      );
    }
  }
  status(format, ...args) {
    const nextStatusStr = this.#nextStatusStr;
    const statusStr = _util.default.format(format, ...args);
    this.#nextStatusStr = this.#ttyStream
      ? chunkString(statusStr, this.#ttyStream.columns).join("\n")
      : statusStr;
    this.#scheduleUpdate();
    return nextStatusStr;
  }
  log(format, ...args) {
    this.#logLines.push(_util.default.format(format, ...args));
    this.#scheduleUpdate();
  }
  persistStatus() {
    this.log(this.#nextStatusStr);
    this.#nextStatusStr = "";
  }
}
exports.default = Terminal;
