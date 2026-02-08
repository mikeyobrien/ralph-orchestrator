"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.transform = void 0;
var _traverse = _interopRequireDefault(require("@babel/traverse"));
var _crypto = _interopRequireDefault(require("crypto"));
var _fs = _interopRequireDefault(require("fs"));
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function asDeserializedBuffer(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value && value.type === "Buffer") {
    return Buffer.from(value.data);
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  return null;
}
const transform = (
  filename,
  transformOptions,
  projectRoot,
  transformerConfig,
  fileBuffer,
) => {
  let data;
  const fileBufferObject = asDeserializedBuffer(fileBuffer);
  if (fileBufferObject) {
    data = fileBufferObject;
  } else {
    data = _fs.default.readFileSync(
      _path.default.resolve(projectRoot, filename),
    );
  }
  return transformFile(
    filename,
    data,
    transformOptions,
    projectRoot,
    transformerConfig,
  );
};
exports.transform = transform;
async function transformFile(
  filename,
  data,
  transformOptions,
  projectRoot,
  transformerConfig,
) {
  const Transformer = require.call(null, transformerConfig.transformerPath);
  const transformFileStartLogEntry = {
    action_name: "Transforming file",
    action_phase: "start",
    file_name: filename,
    log_entry_label: "Transforming file",
    start_timestamp: process.hrtime(),
  };
  const sha1 = _crypto.default.createHash("sha1").update(data).digest("hex");
  const result = await Transformer.transform(
    transformerConfig.transformerConfig,
    projectRoot,
    filename,
    data,
    transformOptions,
  );
  _traverse.default.cache.clear();
  const transformFileEndLogEntry = getEndLogEntry(
    transformFileStartLogEntry,
    filename,
  );
  return {
    result,
    sha1,
    transformFileStartLogEntry,
    transformFileEndLogEntry,
  };
}
function getEndLogEntry(startLogEntry, filename) {
  const timeDelta = process.hrtime(startLogEntry.start_timestamp);
  const duration_ms = Math.round((timeDelta[0] * 1e9 + timeDelta[1]) / 1e6);
  return {
    action_name: "Transforming file",
    action_phase: "end",
    file_name: filename,
    duration_ms,
    log_entry_label: "Transforming file",
  };
}
