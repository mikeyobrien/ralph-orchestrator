"use strict";

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
module.exports = formatFileCandidates;
