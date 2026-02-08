"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = addParamsToDefineCall;
function addParamsToDefineCall(code, ...paramsToAdd) {
  const index = code.lastIndexOf(")");
  const params = paramsToAdd.map((param) =>
    param !== undefined ? JSON.stringify(param) : "undefined",
  );
  return code.slice(0, index) + "," + params.join(",") + code.slice(index);
}
