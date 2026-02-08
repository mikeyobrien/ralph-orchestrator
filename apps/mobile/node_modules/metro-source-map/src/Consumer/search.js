"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.greatestLowerBound = greatestLowerBound;
function greatestLowerBound(elements, target, comparator) {
  let first = 0;
  let it = 0;
  let count = elements.length;
  let step;
  while (count > 0) {
    it = first;
    step = Math.floor(count / 2);
    it = it + step;
    if (comparator(target, elements[it]) >= 0) {
      first = ++it;
      count = count - (step + 1);
    } else {
      count = step;
    }
  }
  return first ? first - 1 : null;
}
