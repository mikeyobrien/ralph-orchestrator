"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.computeHasteConflicts = computeHasteConflicts;
var _constants = _interopRequireDefault(require("../../constants"));
var _sorting = require("../../lib/sorting");
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function computeHasteConflicts({ duplicates, map, rootDir }) {
  const conflicts = [];
  for (const [id, dupsByPlatform] of duplicates.entries()) {
    for (const [platform, conflictingModules] of dupsByPlatform) {
      conflicts.push({
        id,
        platform:
          platform === _constants.default.GENERIC_PLATFORM ? null : platform,
        absolutePaths: [...conflictingModules.keys()]
          .map((modulePath) => _path.default.resolve(rootDir, modulePath))
          .sort(),
        type: "duplicate",
      });
    }
  }
  for (const [id, data] of map) {
    const conflictPaths = new Set();
    const basePaths = [];
    for (const basePlatform of [
      _constants.default.NATIVE_PLATFORM,
      _constants.default.GENERIC_PLATFORM,
    ]) {
      if (data[basePlatform] == null) {
        continue;
      }
      const basePath = data[basePlatform][0];
      basePaths.push(basePath);
      const basePathDir = _path.default.dirname(basePath);
      for (const platform of Object.keys(data)) {
        if (
          platform === basePlatform ||
          platform === _constants.default.GENERIC_PLATFORM
        ) {
          continue;
        }
        const platformPath = data[platform][0];
        if (_path.default.dirname(platformPath) !== basePathDir) {
          conflictPaths.add(platformPath);
        }
      }
    }
    if (conflictPaths.size) {
      conflicts.push({
        id,
        platform: null,
        absolutePaths: [...new Set([...conflictPaths, ...basePaths])]
          .map((modulePath) => _path.default.resolve(rootDir, modulePath))
          .sort(),
        type: "shadowing",
      });
    }
  }
  conflicts.sort(
    (0, _sorting.chainComparators)(
      (a, b) => (0, _sorting.compareStrings)(a.type, b.type),
      (a, b) => (0, _sorting.compareStrings)(a.id, b.id),
      (a, b) => (0, _sorting.compareStrings)(a.platform, b.platform),
    ),
  );
  return conflicts;
}
