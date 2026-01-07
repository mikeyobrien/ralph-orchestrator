"use strict";

const encode = require("./encode");
const MAX_SEGMENT_LENGTH = 7;
const ONE_MEG = 1024 * 1024;
const COMMA = 0x2c;
const SEMICOLON = 0x3b;
class B64Builder {
  constructor() {
    this.buffer = Buffer.alloc(ONE_MEG);
    this.pos = 0;
    this.hasSegment = false;
  }
  markLines(n) {
    if (n < 1) {
      return this;
    }
    this.hasSegment = false;
    if (this.pos + n >= this.buffer.length) {
      this._realloc();
    }
    while (n--) {
      this.buffer[this.pos++] = SEMICOLON;
    }
    return this;
  }
  startSegment(column) {
    if (this.hasSegment) {
      this._writeByte(COMMA);
    } else {
      this.hasSegment = true;
    }
    this.append(column);
    return this;
  }
  append(value) {
    if (this.pos + MAX_SEGMENT_LENGTH >= this.buffer.length) {
      this._realloc();
    }
    this.pos = encode(value, this.buffer, this.pos);
    return this;
  }
  toString() {
    return this.buffer.toString("ascii", 0, this.pos);
  }
  _writeByte(byte) {
    if (this.pos === this.buffer.length) {
      this._realloc();
    }
    this.buffer[this.pos++] = byte;
  }
  _realloc() {
    const { buffer } = this;
    this.buffer = Buffer.alloc(buffer.length * 2);
    buffer.copy(this.buffer);
  }
}
module.exports = B64Builder;
