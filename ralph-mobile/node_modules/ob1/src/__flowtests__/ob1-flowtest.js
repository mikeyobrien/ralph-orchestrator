"use strict";

const { add, add0, add1, get0, get1, inc, neg, sub, sub1 } = require("../ob1");
const FORTY_TWO_0 = add0(42);
const FORTY_TWO_1 = add1(42);
module.exports = {
  testSafeOps() {
    add(FORTY_TWO_0, FORTY_TWO_0);
    add(FORTY_TWO_0, FORTY_TWO_1);
    add(FORTY_TWO_1, FORTY_TWO_0);
    sub(FORTY_TWO_1, FORTY_TWO_1);
    add(FORTY_TWO_0, 9000);
    add(FORTY_TWO_0, 9000);
    add(FORTY_TWO_1, 9000);
    sub(FORTY_TWO_1, 9000);
    get0(FORTY_TWO_0);
    get1(FORTY_TWO_1);
    neg(FORTY_TWO_0);
    add1(FORTY_TWO_0);
    sub1(FORTY_TWO_1);
    inc(FORTY_TWO_0);
    inc(FORTY_TWO_1);
  },
  testUnsafeOps() {
    add(FORTY_TWO_1, FORTY_TWO_1);
    sub(FORTY_TWO_0, FORTY_TWO_1);
    FORTY_TWO_0 - 1;
    FORTY_TWO_1 - 1;
    get0(FORTY_TWO_1);
    get1(FORTY_TWO_0);
    neg(FORTY_TWO_1);
    add1(FORTY_TWO_1);
    sub1(FORTY_TWO_0);
    get0(42);
    get1(42);
  },
};
