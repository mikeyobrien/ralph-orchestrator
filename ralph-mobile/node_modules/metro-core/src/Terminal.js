"use strict";

const throttle = require("lodash.throttle");
const readline = require("readline");
const tty = require("tty");
const util = require("util");
function clearStringBackwards(stream, str) {
  readline.moveCursor(stream, -stream.columns, 0);
  readline.clearLine(stream, 0);
  let lineCount = (str.match(/\n/g) || []).length;
  while (lineCount > 0) {
    readline.moveCursor(stream, 0, -1);
    readline.clearLine(stream, 0);
    --lineCount;
  }
}
function chunkString(str, size) {
  const ANSI_COLOR = "\x1B\\[([0-9]{1,2}(;[0-9]{1,2})?)?m";
  const SKIP_ANSI = `(?:${ANSI_COLOR})*`;
  return str.match(new RegExp(`(?:${SKIP_ANSI}.){1,${size}}`, "g")) || [];
}
function getTTYStream(stream) {
  if (
    stream instanceof tty.WriteStream &&
    stream.isTTY &&
    stream.columns >= 1
  ) {
    return stream;
  }
  return null;
}
class Terminal {
  constructor(stream) {
    this._logLines = [];
    this._nextStatusStr = "";
    this._scheduleUpdate = throttle(this._update, 33);
    this._statusStr = "";
    this._stream = stream;
  }
  _update() {
    const { _statusStr, _stream } = this;
    const ttyStream = getTTYStream(_stream);
    if (_statusStr === this._nextStatusStr && this._logLines.length === 0) {
      return;
    }
    if (ttyStream != null) {
      clearStringBackwards(ttyStream, _statusStr);
    }
    this._logLines.forEach((line) => {
      _stream.write(line);
      _stream.write("\n");
    });
    this._logLines = [];
    if (ttyStream != null) {
      this._nextStatusStr = chunkString(
        this._nextStatusStr,
        ttyStream.columns
      ).join("\n");
      _stream.write(this._nextStatusStr);
    }
    this._statusStr = this._nextStatusStr;
  }
  status(format, ...args) {
    const { _nextStatusStr } = this;
    this._nextStatusStr = util.format(format, ...args);
    this._scheduleUpdate();
    return _nextStatusStr;
  }
  log(format, ...args) {
    this._logLines.push(util.format(format, ...args));
    this._scheduleUpdate();
  }
  persistStatus() {
    this.log(this._nextStatusStr);
    this._nextStatusStr = "";
  }
  flush() {
    this._scheduleUpdate.flush();
  }
}
module.exports = Terminal;
