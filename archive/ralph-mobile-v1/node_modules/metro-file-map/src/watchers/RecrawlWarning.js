"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
class RecrawlWarning {
  static RECRAWL_WARNINGS = [];
  static REGEXP =
    /Recrawled this watch (\d+) times?, most recently because:\n([^:]+)/;
  constructor(root, count) {
    this.root = root;
    this.count = count;
  }
  static findByRoot(root) {
    for (let i = 0; i < this.RECRAWL_WARNINGS.length; i++) {
      const warning = this.RECRAWL_WARNINGS[i];
      if (warning.root === root) {
        return warning;
      }
    }
    return undefined;
  }
  static isRecrawlWarningDupe(warningMessage) {
    if (typeof warningMessage !== "string") {
      return false;
    }
    const match = warningMessage.match(this.REGEXP);
    if (!match) {
      return false;
    }
    const count = Number(match[1]);
    const root = match[2];
    const warning = this.findByRoot(root);
    if (warning) {
      if (warning.count >= count) {
        return true;
      } else {
        warning.count = count;
        return false;
      }
    } else {
      this.RECRAWL_WARNINGS.push(new RecrawlWarning(root, count));
      return false;
    }
  }
}
exports.default = RecrawlWarning;
