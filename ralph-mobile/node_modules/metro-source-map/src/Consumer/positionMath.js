"use strict";

const { add, add0, add1, neg } = require("ob1");
function shiftPositionByOffset(pos, offset) {
  return {
    ...pos,
    line: pos.line != null ? add(pos.line, offset.lines) : null,
    column: pos.column != null ? add(pos.column, offset.columns) : null,
  };
}
function subtractOffsetFromPosition(pos, offset) {
  if (pos.line === add1(offset.lines)) {
    return shiftPositionByOffset(pos, {
      lines: neg(offset.lines),
      columns: neg(offset.columns),
    });
  }
  return shiftPositionByOffset(pos, {
    lines: neg(offset.lines),
    columns: add0(0),
  });
}
module.exports = {
  shiftPositionByOffset,
  subtractOffsetFromPosition,
};
