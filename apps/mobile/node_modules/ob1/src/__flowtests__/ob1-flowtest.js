"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _ob = require("../ob1");
const FORTY_TWO_0 = (0, _ob.add0)(42);
const FORTY_TWO_1 = (0, _ob.add1)(42);
var _default = (exports.default = {
  testSafeOps() {
    (0, _ob.add)(FORTY_TWO_0, FORTY_TWO_0);
    (0, _ob.add)(FORTY_TWO_0, FORTY_TWO_1);
    (0, _ob.add)(FORTY_TWO_1, FORTY_TWO_0);
    (0, _ob.sub)(FORTY_TWO_1, FORTY_TWO_1);
    (0, _ob.add)(FORTY_TWO_0, 9000);
    (0, _ob.add)(FORTY_TWO_0, 9000);
    (0, _ob.add)(FORTY_TWO_1, 9000);
    (0, _ob.sub)(FORTY_TWO_1, 9000);
    (0, _ob.get0)(FORTY_TWO_0);
    (0, _ob.get1)(FORTY_TWO_1);
    (0, _ob.neg)(FORTY_TWO_0);
    (0, _ob.add1)(FORTY_TWO_0);
    (0, _ob.sub1)(FORTY_TWO_1);
    (0, _ob.inc)(FORTY_TWO_0);
    (0, _ob.inc)(FORTY_TWO_1);
  },
  testUnsafeOps() {
    (0, _ob.add)(FORTY_TWO_1, FORTY_TWO_1);
    (0, _ob.sub)(FORTY_TWO_0, FORTY_TWO_1);
    FORTY_TWO_0 - 1;
    FORTY_TWO_1 - 1;
    (0, _ob.get0)(FORTY_TWO_1);
    (0, _ob.get1)(FORTY_TWO_0);
    (0, _ob.neg)(FORTY_TWO_1);
    (0, _ob.add1)(FORTY_TWO_1);
    (0, _ob.sub1)(FORTY_TWO_0);
    (0, _ob.get0)(42);
    (0, _ob.get1)(42);
  },
});
