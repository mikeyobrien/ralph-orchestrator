"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.comparableCode = exports.codeFromAst = void 0;
exports.toEqualComparableCode = toEqualComparableCode;
exports.toMatchCodeFrameSnapshot = toMatchCodeFrameSnapshot;
var _generator = _interopRequireDefault(require("@babel/generator"));
var _jestSnapshot = require("jest-snapshot");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const generateOptions = {
  concise: true,
  sourceType: "module",
};
const codeFromAst = (ast) => (0, _generator.default)(ast, generateOptions).code;
exports.codeFromAst = codeFromAst;
const comparableCode = (code) => code.trim().replace(/\s+/g, " ");
exports.comparableCode = comparableCode;
function toEqualComparableCode(received, expected) {
  const comparableExpected = comparableCode(expected);
  const pass = received === comparableExpected;
  const options = {
    isNot: this.isNot,
    promise: this.promise,
  };
  const message = pass
    ? () =>
        this.utils.matcherHint(
          "toEqualComparableCode",
          undefined,
          undefined,
          options,
        ) +
        "\n\n" +
        `Expected: not ${this.utils.printExpected(comparableExpected)}\n` +
        `Received: ${this.utils.printReceived(received)}`
    : () => {
        const diffString = this.utils.printDiffOrStringify(
          comparableExpected,
          received,
          "expected",
          "received",
          this.expand,
        );
        return (
          this.utils.matcherHint(
            "toEqualComparableCode",
            undefined,
            undefined,
            options,
          ) +
          "\n\n" +
          diffString
        );
      };
  return {
    actual: received,
    message,
    pass,
  };
}
const ANSI_PATTERN = /\x1b\[[0-9;]*m/g;
function trimANSICodes(input) {
  return input.replace(ANSI_PATTERN, "");
}
function toMatchCodeFrameSnapshot(received) {
  return _jestSnapshot.toMatchSnapshot.call(
    this,
    trimANSICodes(received),
    "toMatchCodeFrameSnapshot",
  );
}
