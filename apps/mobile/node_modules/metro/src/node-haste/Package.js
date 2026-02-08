"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _fs = _interopRequireDefault(require("fs"));
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class Package {
  constructor({ file }) {
    this.path = _path.default.resolve(file);
    this._root = _path.default.dirname(this.path);
    this._content = null;
  }
  invalidate() {
    this._content = null;
  }
  read() {
    if (this._content == null) {
      this._content = JSON.parse(_fs.default.readFileSync(this.path, "utf8"));
    }
    return this._content;
  }
}
exports.default = Package;
