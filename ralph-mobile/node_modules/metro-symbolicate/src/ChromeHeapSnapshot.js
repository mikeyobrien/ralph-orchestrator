"use strict";

const invariant = require("invariant");
const CHILDREN_FIELD_TYPE = "__CHILDREN__";
class ChromeHeapSnapshotProcessor {
  constructor(snapshotData) {
    this._snapshotData = snapshotData;
    this._globalStringTable = new ChromeHeapSnapshotStringTable(
      this._snapshotData.strings
    );
  }
  traceFunctionInfos() {
    return new ChromeHeapSnapshotRecordIterator(
      this._snapshotData.trace_function_infos,
      this._snapshotData.snapshot.meta.trace_function_info_fields,
      {
        name: "string",
        script_name: "string",
      },
      this._globalStringTable,
      undefined
    );
  }
  locations() {
    return new ChromeHeapSnapshotRecordIterator(
      this._snapshotData.locations,
      this._snapshotData.snapshot.meta.location_fields,
      null,
      this._globalStringTable,
      undefined
    );
  }
  nodes() {
    return new ChromeHeapSnapshotRecordIterator(
      this._snapshotData.nodes,
      this._snapshotData.snapshot.meta.node_fields,
      this._snapshotData.snapshot.meta.node_types,
      this._globalStringTable,
      undefined
    );
  }
  edges() {
    return new ChromeHeapSnapshotRecordIterator(
      this._snapshotData.edges,
      this._snapshotData.snapshot.meta.edge_fields,
      this._snapshotData.snapshot.meta.edge_types,
      this._globalStringTable,
      undefined
    );
  }
  traceTree() {
    return new ChromeHeapSnapshotRecordIterator(
      this._snapshotData.trace_tree,
      this._snapshotData.snapshot.meta.trace_node_fields,
      {
        children: CHILDREN_FIELD_TYPE,
      },
      this._globalStringTable,
      undefined
    );
  }
}
class ChromeHeapSnapshotStringTable {
  constructor(strings) {
    this._strings = strings;
    this._indexCache = new Map();
  }
  add(value) {
    this._syncIndexCache();
    let index = this._indexCache.get(value);
    if (index != null) {
      return index;
    }
    index = this._strings.length;
    this._strings.push(value);
    this._indexCache.set(value, index);
    return index;
  }
  get(index) {
    invariant(
      index >= 0 && index < this._strings.length,
      "index out of string table range"
    );
    return this._strings[index];
  }
  _syncIndexCache() {
    if (this._strings.length > this._indexCache.size) {
      for (let i = this._indexCache.size; i < this._strings.length; ++i) {
        this._indexCache.set(this._strings[i], i);
      }
    }
  }
}
class ChromeHeapSnapshotRecordAccessor {
  constructor(
    buffer,
    recordFields,
    recordTypes,
    globalStringTable,
    position,
    parent
  ) {
    if (parent) {
      this._recordSize = parent._recordSize;
      this._fieldToOffset = parent._fieldToOffset;
      this._fieldToType = parent._fieldToType;
    } else {
      this._recordSize = recordFields.length;
      this._fieldToOffset = new Map(
        Object.entries(recordFields).map(([offsetStr, name]) => [
          String(name),
          Number(offsetStr),
        ])
      );
      if (Array.isArray(recordTypes)) {
        this._fieldToType = new Map(
          Object.entries(recordTypes).map(([offsetStr, type]) => [
            recordFields[Number(offsetStr)],
            type,
          ])
        );
      } else {
        this._fieldToType = new Map(Object.entries(recordTypes || {}));
      }
    }
    this._buffer = buffer;
    this._position = position;
    invariant(
      this._position % this._recordSize === 0,
      "Record accessor constructed at invalid offset"
    );
    invariant(
      this._buffer.length % this._recordSize === 0,
      "Record accessor constructed with wrong size buffer"
    );
    this._globalStringTable = globalStringTable;
  }
  getString(field) {
    const dynamicValue = this._getScalar(field);
    if (typeof dynamicValue === "string") {
      return dynamicValue;
    }
    throw new Error("Not a string or enum field: " + field);
  }
  getNumber(field) {
    const dynamicValue = this._getScalar(field);
    if (typeof dynamicValue === "number") {
      return dynamicValue;
    }
    throw new Error("Not a number field: " + field);
  }
  getChildren(field) {
    const fieldType = this._fieldToType.get(field);
    if (fieldType !== CHILDREN_FIELD_TYPE) {
      throw new Error("Not a children field: " + field);
    }
    const childrenBuffer = this._getRaw(field);
    invariant(
      Array.isArray(childrenBuffer),
      "Expected array in children-typed field"
    );
    return new ChromeHeapSnapshotRecordIterator(
      childrenBuffer,
      [],
      null,
      this._globalStringTable,
      -this._fieldToOffset.size,
      this
    );
  }
  setString(field, value) {
    this._setRaw(field, this._encodeString(field, value));
  }
  setNumber(field, value) {
    const fieldType = this._fieldToType.get(field);
    if (
      Array.isArray(fieldType) ||
      fieldType === "string" ||
      fieldType === CHILDREN_FIELD_TYPE
    ) {
      throw new Error("Not a number field: " + field);
    }
    this._setRaw(field, value);
  }
  moveToRecord(recordIndex) {
    this._moveToPosition(recordIndex * this._recordSize);
  }
  append(record) {
    const savedPosition = this._position;
    try {
      return this.moveAndInsert(this._buffer.length / this._recordSize, record);
    } finally {
      this._position = savedPosition;
    }
  }
  moveAndInsert(recordIndex, record) {
    this._moveToPosition(recordIndex * this._recordSize, true);
    let didResizeBuffer = false;
    try {
      for (const field of this._fieldToOffset.keys()) {
        if (!Object.prototype.hasOwnProperty.call(record, field)) {
          throw new Error("Missing value for field: " + field);
        }
      }
      this._buffer.splice(this._position, 0, ...new Array(this._recordSize));
      didResizeBuffer = true;
      for (const field of Object.keys(record)) {
        this._set(field, record[field]);
      }
      return this._position / this._recordSize;
    } catch (e) {
      if (didResizeBuffer) {
        this._buffer.splice(this._position, this._recordSize);
      }
      throw e;
    }
  }
  protectedHasNext() {
    if (this._position < 0) {
      return this._buffer.length > 0;
    }
    return this._position < this._buffer.length;
  }
  protectedTryMoveNext() {
    if (this.protectedHasNext()) {
      this._moveToPosition(this._position + this._recordSize, true);
    }
  }
  _getRaw(field) {
    this._validatePosition();
    const offset = this._fieldToOffset.get(field);
    if (offset == null) {
      throw new Error("Unknown field: " + field);
    }
    return this._buffer[this._position + offset];
  }
  _getScalar(field) {
    const rawValue = this._getRaw(field);
    if (Array.isArray(rawValue)) {
      throw new Error("Not a scalar field: " + field);
    }
    const fieldType = this._fieldToType.get(field);
    if (Array.isArray(fieldType)) {
      invariant(
        rawValue >= 0 && rawValue < fieldType.length,
        "raw value does not match field enum type"
      );
      return fieldType[rawValue];
    }
    if (fieldType === "string") {
      return this._globalStringTable.get(rawValue);
    }
    return rawValue;
  }
  _setRaw(field, rawValue) {
    this._validatePosition();
    const offset = this._fieldToOffset.get(field);
    if (offset == null) {
      throw new Error("Unknown field: " + field);
    }
    this._buffer[this._position + offset] = rawValue;
  }
  _set(field, value) {
    if (typeof value === "string") {
      this.setString(field, value);
    } else if (typeof value === "number") {
      this.setNumber(field, value);
    } else if (Array.isArray(value)) {
      this._setChildren(field, value);
    } else {
      throw new Error("Unsupported value for field: " + field);
    }
  }
  _setChildren(field, value) {
    const fieldType = this._fieldToType.get(field);
    if (fieldType !== CHILDREN_FIELD_TYPE) {
      throw new Error("Not a children field: " + field);
    }
    this._setRaw(field, []);
    const childIt = this.getChildren(field);
    for (const child of value) {
      childIt.append(child);
    }
  }
  _encodeString(field, value) {
    const fieldType = this._fieldToType.get(field);
    if (Array.isArray(fieldType)) {
      const index = fieldType.indexOf(value);
      invariant(index >= 0, "Cannot define new values in enum field");
      return index;
    }
    if (fieldType === "string") {
      return this._globalStringTable.add(value);
    }
    throw new Error("Not a string or enum field: " + field);
  }
  _validatePosition(allowEnd = false, position = this._position) {
    if (!Number.isInteger(position)) {
      throw new Error(`Position ${position} is not an integer`);
    }
    if (position % this._recordSize !== 0) {
      throw new Error(
        `Position ${position} is not a multiple of record size ${this._recordSize}`
      );
    }
    if (position < 0) {
      throw new Error(`Position ${position} is out of range`);
    }
    const maxPosition = allowEnd
      ? this._buffer.length
      : this._buffer.length - 1;
    if (position > maxPosition) {
      throw new Error(`Position ${position} is out of range`);
    }
    if (this._buffer.length - position < this._recordSize) {
      if (!(allowEnd && this._buffer.length === position)) {
        throw new Error(
          `Record at position ${position} is truncated: expected ${
            this._recordSize
          } fields but found ${this._buffer.length - position}`
        );
      }
    }
  }
  _moveToPosition(nextPosition, allowEnd = false) {
    this._validatePosition(allowEnd, nextPosition);
    this._position = nextPosition;
  }
}
class ChromeHeapSnapshotRecordIterator extends ChromeHeapSnapshotRecordAccessor {
  constructor(
    buffer,
    recordFields,
    recordTypes,
    globalStringTable,
    position = -recordFields.length,
    parent
  ) {
    super(
      buffer,
      recordFields,
      recordTypes,
      globalStringTable,
      position,
      parent
    );
  }
  next() {
    this.protectedTryMoveNext();
    return {
      done: !this.protectedHasNext(),
      value: this,
    };
  }
  [Symbol.iterator]() {
    return this;
  }
}
module.exports = {
  ChromeHeapSnapshotProcessor,
};
