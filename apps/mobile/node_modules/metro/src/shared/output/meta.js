"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = _default;
var _crypto = _interopRequireDefault(require("crypto"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const isUTF8 = (encoding) => /^utf-?8$/i.test(encoding);
const constantFor = (encoding) =>
  /^ascii$/i.test(encoding)
    ? 1
    : isUTF8(encoding)
      ? 2
      : /^(?:utf-?16(?:le)?|ucs-?2)$/.test(encoding)
        ? 3
        : 0;
function _default(code, encoding = "utf8") {
  const buffer = asBuffer(code, encoding);
  const hash = _crypto.default.createHash("sha1");
  hash.update(buffer);
  const digest = hash.digest("buffer");
  const signature = Buffer.alloc(digest.length + 1);
  digest.copy(signature);
  signature.writeUInt8(
    constantFor(tryAsciiPromotion(buffer, encoding)),
    signature.length - 1,
  );
  return signature;
}
function tryAsciiPromotion(buffer, encoding) {
  if (!isUTF8(encoding)) {
    return encoding;
  }
  for (let i = 0, n = buffer.length; i < n; i++) {
    if (buffer[i] > 0x7f) {
      return encoding;
    }
  }
  return "ascii";
}
function asBuffer(x, encoding) {
  if (typeof x !== "string") {
    return x;
  }
  return Buffer.from(x, encoding);
}
