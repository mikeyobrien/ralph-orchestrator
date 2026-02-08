"use strict";

const constants = {
  DEPENDENCY_DELIM: "\0",
  MTIME: 0,
  SIZE: 1,
  VISITED: 2,
  DEPENDENCIES: 3,
  SHA1: 4,
  SYMLINK: 5,
  ID: 6,
  PATH: 0,
  TYPE: 1,
  MODULE: 0,
  PACKAGE: 1,
  GENERIC_PLATFORM: "g",
  NATIVE_PLATFORM: "native",
};
module.exports = constants;
