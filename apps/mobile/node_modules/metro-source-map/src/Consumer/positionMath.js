"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.shiftPositionByOffset = shiftPositionByOffset;
exports.subtractOffsetFromPosition = subtractOffsetFromPosition;
var _ob = require("ob1");
function shiftPositionByOffset(pos, offset) {
  return {
    ...pos,
    line: pos.line != null ? (0, _ob.add)(pos.line, offset.lines) : null,
    column:
      pos.column != null ? (0, _ob.add)(pos.column, offset.columns) : null,
  };
}
function subtractOffsetFromPosition(pos, offset) {
  if (pos.line === (0, _ob.add1)(offset.lines)) {
    return shiftPositionByOffset(pos, {
      lines: (0, _ob.neg)(offset.lines),
      columns: (0, _ob.neg)(offset.columns),
    });
  }
  return shiftPositionByOffset(pos, {
    lines: (0, _ob.neg)(offset.lines),
    columns: (0, _ob.add0)(0),
  });
}
