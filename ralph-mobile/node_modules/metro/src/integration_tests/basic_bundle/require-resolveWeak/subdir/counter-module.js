"use strict";

let count = 0;
module.exports = {
  increment() {
    ++count;
    return count;
  },
};
