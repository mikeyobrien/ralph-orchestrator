"use strict";

const Bar = require("./Bar");
const Foo = require("./Foo");
const TypeScript = require("./TypeScript");
Object.keys({
  ...Bar,
});
module.exports = {
  Foo,
  Bar,
  TypeScript,
};
