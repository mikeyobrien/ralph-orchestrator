"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = getPlatformExtension;
function getPlatformExtension(file, platforms) {
  const last = file.lastIndexOf(".");
  const secondToLast = file.lastIndexOf(".", last - 1);
  if (secondToLast === -1) {
    return null;
  }
  const platform = file.substring(secondToLast + 1, last);
  return platforms.has(platform) ? platform : null;
}
