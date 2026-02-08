"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _AbstractConsumer = _interopRequireDefault(require("./AbstractConsumer"));
var _constants = require("./constants");
var _createConsumer = _interopRequireDefault(require("./createConsumer"));
var _positionMath = require("./positionMath");
var _search = require("./search");
var _ob = require("ob1");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
class SectionsConsumer extends _AbstractConsumer.default {
  constructor(sourceMap) {
    super(sourceMap);
    this._consumers = sourceMap.sections.map((section, index) => {
      const generatedOffset = {
        lines: (0, _ob.add0)(section.offset.line),
        columns: (0, _ob.add0)(section.offset.column),
      };
      const consumer = (0, _createConsumer.default)(section.map);
      return [generatedOffset, consumer];
    });
  }
  originalPositionFor(generatedPosition) {
    const [generatedOffset, consumer] =
      this._consumerForPosition(generatedPosition) || [];
    if (!consumer) {
      return {
        ..._constants.EMPTY_POSITION,
      };
    }
    return consumer.originalPositionFor(
      (0, _positionMath.subtractOffsetFromPosition)(
        generatedPosition,
        generatedOffset,
      ),
    );
  }
  *generatedMappings() {
    for (const [generatedOffset, consumer] of this._consumers) {
      let first = true;
      for (const mapping of consumer.generatedMappings()) {
        if (
          first &&
          ((0, _ob.get1)(mapping.generatedLine) > 1 ||
            (0, _ob.get0)(mapping.generatedColumn) > 0)
        ) {
          yield {
            generatedLine: _constants.FIRST_LINE,
            generatedColumn: _constants.FIRST_COLUMN,
            source: null,
            name: null,
            originalLine: null,
            originalColumn: null,
          };
        }
        first = false;
        yield {
          ...mapping,
          generatedLine: (0, _ob.add)(
            mapping.generatedLine,
            generatedOffset.lines,
          ),
          generatedColumn: (0, _ob.add)(
            mapping.generatedColumn,
            generatedOffset.columns,
          ),
        };
      }
    }
  }
  _consumerForPosition(generatedPosition) {
    const { line, column } = generatedPosition;
    if (line == null || column == null) {
      return null;
    }
    const index = (0, _search.greatestLowerBound)(
      this._consumers,
      generatedPosition,
      (position, [offset]) => {
        const line0 = (0, _ob.sub1)(line);
        const column0 = column;
        if (line0 === offset.lines) {
          return (0, _ob.get0)((0, _ob.sub)(column0, offset.columns));
        }
        return (0, _ob.get0)((0, _ob.sub)(line0, offset.lines));
      },
    );
    return index != null ? this._consumers[index] : null;
  }
  sourceContentFor(source, nullOnMissing) {
    for (const [_, consumer] of this._consumers) {
      const content = consumer.sourceContentFor(source, nullOnMissing);
      if (content != null) {
        return content;
      }
    }
    return null;
  }
}
exports.default = SectionsConsumer;
