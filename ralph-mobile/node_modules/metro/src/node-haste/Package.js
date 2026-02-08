"use strict";

const fs = require("fs");
const path = require("path");
class Package {
  constructor({ file }) {
    this.path = path.resolve(file);
    this._root = path.dirname(this.path);
    this._content = null;
  }
  invalidate() {
    this._content = null;
  }
  read() {
    if (this._content == null) {
      this._content = JSON.parse(fs.readFileSync(this.path, "utf8"));
    }
    return this._content;
  }
}
module.exports = Package;
