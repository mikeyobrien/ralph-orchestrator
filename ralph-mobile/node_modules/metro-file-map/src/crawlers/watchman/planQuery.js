"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.planQuery = planQuery;
function planQuery({
  since,
  directoryFilters,
  extensions,
  includeSha1,
  includeSymlinks,
}) {
  const fields = ["name", "exists", "mtime_ms", "size"];
  if (includeSha1) {
    fields.push("content.sha1hex");
  }
  if (includeSymlinks) {
    fields.push("type");
  }
  const allOfTerms = includeSymlinks
    ? [
        [
          "anyof",
          ["allof", ["type", "f"], ["suffix", extensions]],
          ["type", "l"],
        ],
      ]
    : [["type", "f"]];
  const query = {
    fields,
  };
  let queryGenerator;
  if (since != null) {
    query.since = since;
    queryGenerator = "since";
    if (directoryFilters.length > 0) {
      allOfTerms.push([
        "anyof",
        ...directoryFilters.map((dir) => ["dirname", dir]),
      ]);
    }
  } else if (directoryFilters.length > 0) {
    query.glob = directoryFilters.map((directory) => `${directory}/**`);
    query.glob_includedotfiles = true;
    queryGenerator = "glob";
  } else if (!includeSymlinks) {
    query.suffix = extensions;
    queryGenerator = "suffix";
  } else {
    queryGenerator = "all";
  }
  if (!includeSymlinks && queryGenerator !== "suffix") {
    allOfTerms.push(["suffix", extensions]);
  }
  query.expression =
    allOfTerms.length === 1 ? allOfTerms[0] : ["allof", ...allOfTerms];
  return {
    query,
    queryGenerator,
  };
}
