"use strict";

function getMinifier(minifierPath) {
  try {
    return require(minifierPath);
  } catch (e) {
    throw new Error(
      'A problem occurred while trying to fetch the minifier. Path: "' +
        minifierPath +
        '", error message: ' +
        e.message
    );
  }
}
module.exports = getMinifier;
