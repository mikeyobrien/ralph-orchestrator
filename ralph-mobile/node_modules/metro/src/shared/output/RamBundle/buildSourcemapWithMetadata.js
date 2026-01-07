"use strict";

const {
  combineSourceMaps,
  combineSourceMapsAddingOffsets,
  joinModules,
} = require("./util");
module.exports = ({
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
    code: joinModules(startupModules),
    id: Number.MIN_SAFE_INTEGER,
    map: combineSourceMaps(startupModules, undefined, options),
    sourcePath: "",
  };
  const module_paths = [];
  startupModules.forEach((m) => {
    module_paths[m.id] = m.sourcePath;
  });
  lazyModules.forEach((m) => {
    module_paths[m.id] = m.sourcePath;
  });
  const map = combineSourceMapsAddingOffsets(
    [startupModule].concat(lazyModules),
    module_paths,
    moduleGroups,
    options
  );
  if (map.x_facebook_offsets != null) {
    delete map.x_facebook_offsets[Number.MIN_SAFE_INTEGER];
  }
  return map;
};
