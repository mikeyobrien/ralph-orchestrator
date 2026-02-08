"use strict";

class FailedToResolveNameError extends Error {
  constructor(dirPaths, extraPaths) {
    const displayDirPaths = dirPaths.concat(extraPaths);
    const hint = displayDirPaths.length ? " or in these directories:" : "";
    super(
      `Module does not exist in the Haste module map${hint}\n` +
        displayDirPaths.map((dirPath) => `  ${dirPath}`).join("\n") +
        "\n"
    );
    this.dirPaths = dirPaths;
    this.extraPaths = extraPaths;
  }
}
module.exports = FailedToResolveNameError;
