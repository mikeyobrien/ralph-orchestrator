"use strict";

const constants = {
  DEPENDENCY_DELIM: "\0",
  ID: 0,
  MTIME: 1,
  SIZE: 2,
  VISITED: 3,
  DEPENDENCIES: 4,
  SHA1: 5,
  SYMLINK: 6,
  PATH: 0,
  TYPE: 1,
  MODULE: 0,
  PACKAGE: 1,
  GENERIC_PLATFORM: "g",
  NATIVE_PLATFORM: "native",
};
module.exports = constants;
