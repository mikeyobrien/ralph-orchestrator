"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = formatFileCandidates;
function formatFileCandidates(candidates) {
  if (candidates.type === "asset") {
    return candidates.name;
  }
  let formatted = candidates.filePathPrefix;
  if (candidates.candidateExts.length) {
    formatted += "(" + candidates.candidateExts.filter(Boolean).join("|") + ")";
  }
  return formatted;
}
