"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _util = require("./util");
var _default = ({
  fixWrapperOffset,
  lazyModules,
  moduleGroups,
  startupModules,
}) => {
  const options = fixWrapperOffset
    ? {
        fixWrapperOffset: true,
      }
    : undefined;
  const startupModule = {
    code: (0, _util.joinModules)(startupModules),
    id: Number.MIN_SAFE_INTEGER,
    map: (0, _util.combineSourceMaps)(startupModules, undefined, options),
    sourcePath: "",
  };
  const module_paths = [];
  startupModules.forEach((m) => {
    module_paths[m.id] = m.sourcePath;
  });
  lazyModules.forEach((m) => {
    module_paths[m.id] = m.sourcePath;
  });
  const map = (0, _util.combineSourceMapsAddingOffsets)(
    [startupModule].concat(lazyModules),
    module_paths,
    moduleGroups,
    options,
  );
  if (map.x_facebook_offsets != null) {
    delete map.x_facebook_offsets[Number.MIN_SAFE_INTEGER];
  }
  return map;
};
exports.default = _default;
