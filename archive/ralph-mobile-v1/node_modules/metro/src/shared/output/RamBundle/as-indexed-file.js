"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.buildTableAndContents = buildTableAndContents;
exports.createModuleGroups = createModuleGroups;
exports.save = save;
var _relativizeSourceMap = _interopRequireDefault(
  require("../../../lib/relativizeSourceMap"),
);
var _buildSourcemapWithMetadata = _interopRequireDefault(
  require("./buildSourcemapWithMetadata"),
);
var _magicNumber = _interopRequireDefault(require("./magic-number"));
var _util = require("./util");
var _writeSourcemap = _interopRequireDefault(require("./write-sourcemap"));
var _fs = _interopRequireDefault(require("fs"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const SIZEOF_UINT32 = 4;
function save(bundle, options, log) {
  const {
    bundleOutput,
    bundleEncoding: encoding,
    sourcemapOutput,
    sourcemapSourcesRoot,
  } = options;
  log("start");
  const { startupModules, lazyModules, groups } = bundle;
  log("finish");
  const moduleGroups = createModuleGroups(groups, lazyModules);
  const startupCode = (0, _util.joinModules)(startupModules);
  log("Writing unbundle output to:", bundleOutput);
  const writeUnbundle = writeBuffers(
    _fs.default.createWriteStream(bundleOutput),
    buildTableAndContents(startupCode, lazyModules, moduleGroups, encoding),
  ).then(() => log("Done writing unbundle output"));
  if (sourcemapOutput) {
    const sourceMap = (0, _buildSourcemapWithMetadata.default)({
      startupModules: startupModules.concat(),
      lazyModules: lazyModules.concat(),
      moduleGroups,
      fixWrapperOffset: true,
    });
    if (sourcemapSourcesRoot != null) {
      (0, _relativizeSourceMap.default)(sourceMap, sourcemapSourcesRoot);
    }
    const wroteSourceMap = (0, _writeSourcemap.default)(
      sourcemapOutput,
      JSON.stringify(sourceMap),
      log,
    );
    return Promise.all([writeUnbundle, wroteSourceMap]);
  } else {
    return writeUnbundle;
  }
}
const fileHeader = Buffer.alloc(4);
fileHeader.writeUInt32LE(_magicNumber.default, 0);
const nullByteBuffer = Buffer.alloc(1).fill(0);
function writeBuffers(stream, buffers) {
  buffers.forEach((buffer) => stream.write(buffer));
  return new Promise((resolve, reject) => {
    stream.on("error", reject);
    stream.on("finish", () => resolve());
    stream.end();
  });
}
function nullTerminatedBuffer(contents, encoding) {
  return Buffer.concat([Buffer.from(contents, encoding), nullByteBuffer]);
}
function moduleToBuffer(id, code, encoding) {
  return {
    id,
    buffer: nullTerminatedBuffer(code, encoding),
  };
}
function entryOffset(n) {
  return (2 + n * 2) * SIZEOF_UINT32;
}
function buildModuleTable(startupCode, moduleBuffers, moduleGroups) {
  const moduleIds = [...moduleGroups.modulesById.keys()];
  const maxId = moduleIds.reduce((max, id) => Math.max(max, id));
  const numEntries = maxId + 1;
  const table = Buffer.alloc(entryOffset(numEntries)).fill(0);
  table.writeUInt32LE(numEntries, 0);
  table.writeUInt32LE(startupCode.length, SIZEOF_UINT32);
  let codeOffset = startupCode.length;
  moduleBuffers.forEach(({ id, buffer }) => {
    const group = moduleGroups.groups.get(id);
    const idsInGroup = group ? [id].concat(Array.from(group)) : [id];
    idsInGroup.forEach((moduleId) => {
      const offset = entryOffset(moduleId);
      table.writeUInt32LE(codeOffset, offset);
      table.writeUInt32LE(buffer.length, offset + SIZEOF_UINT32);
    });
    codeOffset += buffer.length;
  });
  return table;
}
function groupCode(rootCode, moduleGroup, modulesById) {
  if (!moduleGroup || !moduleGroup.size) {
    return rootCode;
  }
  const code = [rootCode];
  for (const id of moduleGroup) {
    code.push(
      (
        modulesById.get(id) || {
          code: "",
        }
      ).code,
    );
  }
  return code.join("\n");
}
function buildModuleBuffers(modules, moduleGroups, encoding) {
  return modules
    .filter((m) => !moduleGroups.modulesInGroups.has(m.id))
    .map(({ id, code }) =>
      moduleToBuffer(
        id,
        groupCode(code, moduleGroups.groups.get(id), moduleGroups.modulesById),
        encoding,
      ),
    );
}
function buildTableAndContents(startupCode, modules, moduleGroups, encoding) {
  const startupCodeBuffer = nullTerminatedBuffer(startupCode, encoding);
  const moduleBuffers = buildModuleBuffers(modules, moduleGroups, encoding);
  const table = buildModuleTable(
    startupCodeBuffer,
    moduleBuffers,
    moduleGroups,
  );
  return [fileHeader, table, startupCodeBuffer].concat(
    moduleBuffers.map(({ buffer }) => buffer),
  );
}
function createModuleGroups(groups, modules) {
  return {
    groups,
    modulesById: new Map(modules.map((m) => [m.id, m])),
    modulesInGroups: new Set(concat(groups.values())),
  };
}
function* concat(iterators) {
  for (const it of iterators) {
    yield* it;
  }
}
