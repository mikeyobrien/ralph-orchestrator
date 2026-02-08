"use strict";

const traverse = require("@babel/traverse").default;
const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
function asDeserializedBuffer(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value && value.type === "Buffer") {
    return Buffer.from(value.data);
  }
  return null;
}
async function transform(
  filename,
  transformOptions,
  projectRoot,
  transformerConfig,
  fileBuffer
) {
  let data;
  const fileBufferObject = asDeserializedBuffer(fileBuffer);
  if (fileBufferObject) {
    data = fileBufferObject;
  } else {
    data = fs.readFileSync(path.resolve(projectRoot, filename));
  }
  return transformFile(
    filename,
    data,
    transformOptions,
    projectRoot,
    transformerConfig
  );
}
async function transformFile(
  filename,
  data,
  transformOptions,
  projectRoot,
  transformerConfig
) {
  const Transformer = require.call(null, transformerConfig.transformerPath);
  const transformFileStartLogEntry = {
    action_name: "Transforming file",
    action_phase: "start",
    file_name: filename,
    log_entry_label: "Transforming file",
    start_timestamp: process.hrtime(),
  };
  const sha1 = crypto.createHash("sha1").update(data).digest("hex");
  const result = await Transformer.transform(
    transformerConfig.transformerConfig,
    projectRoot,
    filename,
    data,
    transformOptions
  );
  traverse.cache.clear();
  const transformFileEndLogEntry = getEndLogEntry(
    transformFileStartLogEntry,
    filename
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
module.exports = {
  transform,
};
