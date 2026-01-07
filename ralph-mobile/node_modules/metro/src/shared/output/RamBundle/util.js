"use strict";

var _countLines = _interopRequireDefault(require("../../../lib/countLines"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const invariant = require("invariant");
function lineToLineSourceMap(source, filename = "") {
  const firstLine = "AAAA;";
  const line = "AACA;";
  return {
    file: filename,
    mappings: firstLine + Array((0, _countLines.default)(source)).join(line),
    sources: [filename],
    names: [],
    version: 3,
  };
}
const wrapperEnd = (wrappedCode) => wrappedCode.indexOf("{") + 1;
const Section = (line, column, map) => ({
  map,
  offset: {
    line,
    column,
  },
});
function combineSourceMaps(modules, moduleGroups, options) {
  const sections = combineMaps(modules, null, moduleGroups, options);
  return {
    sections,
    version: 3,
  };
}
function combineSourceMapsAddingOffsets(
  modules,
  x_metro_module_paths,
  moduleGroups,
  options
) {
  const x_facebook_offsets = [];
  const sections = combineMaps(
    modules,
    x_facebook_offsets,
    moduleGroups,
    options
  );
  return {
    sections,
    version: 3,
    x_facebook_offsets,
    x_metro_module_paths,
  };
}
function combineMaps(modules, offsets, moduleGroups, options) {
  const sections = [];
  let line = 0;
  modules.forEach((moduleTransport) => {
    const { code, id, name } = moduleTransport;
    let column = 0;
    let group;
    let groupLines = 0;
    let { map } = moduleTransport;
    if (moduleGroups && moduleGroups.modulesInGroups.has(id)) {
      return;
    }
    if (offsets != null) {
      group = moduleGroups && moduleGroups.groups.get(id);
      if (group && moduleGroups) {
        const { modulesById } = moduleGroups;
        const otherModules = Array.from(group || [])
          .map((moduleId) => modulesById.get(moduleId))
          .filter(Boolean);
        otherModules.forEach((m) => {
          groupLines += (0, _countLines.default)(m.code);
        });
        map = combineSourceMaps([moduleTransport].concat(otherModules));
      }
      column = options && options.fixWrapperOffset ? wrapperEnd(code) : 0;
    }
    invariant(
      !Array.isArray(map),
      "Random Access Bundle source maps cannot be built from raw mappings"
    );
    sections.push(
      Section(line, column, map || lineToLineSourceMap(code, name))
    );
    if (offsets != null && id != null) {
      offsets[id] = line;
      for (const moduleId of group || []) {
        offsets[moduleId] = line;
      }
    }
    line += (0, _countLines.default)(code) + groupLines;
  });
  return sections;
}
const joinModules = (modules) => modules.map((m) => m.code).join("\n");
module.exports = {
  combineSourceMaps,
  combineSourceMapsAddingOffsets,
  countLines: _countLines.default,
  joinModules,
  lineToLineSourceMap,
};
