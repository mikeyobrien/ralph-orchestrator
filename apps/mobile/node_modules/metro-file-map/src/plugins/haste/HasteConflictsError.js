"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.HasteConflictsError = void 0;
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class HasteConflictsError extends Error {
  #conflicts;
  constructor(conflicts) {
    super(
      `Found ${conflicts.length} Haste conflict(s). Haste module IDs must be globally unique in the codebase.`,
    );
    this.#conflicts = conflicts;
  }
  getDetailedMessage(pathsRelativeToRoot) {
    const messages = [];
    const conflicts = this.#conflicts;
    if (conflicts.some((conflict) => conflict.type === "duplicate")) {
      messages.push(
        'Advice: Resolve conflicts of type "duplicate" by renaming one or both of the conflicting modules, or by excluding conflicting paths from Haste.',
      );
    }
    if (conflicts.some((conflict) => conflict.type === "shadowing")) {
      messages.push(
        'Advice: Resolve conflicts of type "shadowing" by moving the modules to the same folder, or by excluding conflicting paths from Haste.',
      );
    }
    let index = 0;
    for (const conflict of conflicts) {
      const itemHeader = index + 1 + ". ";
      const indent = " ".repeat(itemHeader.length + 2);
      messages.push(
        "\n" +
          itemHeader +
          conflict.id +
          (conflict.platform != null ? `.${conflict.platform}` : "") +
          ` (${conflict.type})`,
      );
      for (const modulePath of conflict.absolutePaths) {
        messages.push(
          indent +
            (pathsRelativeToRoot != null
              ? _path.default.relative(pathsRelativeToRoot, modulePath)
              : modulePath),
        );
      }
      ++index;
    }
    return messages.join("\n");
  }
}
exports.HasteConflictsError = HasteConflictsError;
