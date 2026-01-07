"use strict";

const Resolver = {
  FailedToResolveNameError: require("./errors/FailedToResolveNameError"),
  FailedToResolvePathError: require("./errors/FailedToResolvePathError"),
  FailedToResolveUnsupportedError: require("./errors/FailedToResolveUnsupportedError"),
  formatFileCandidates: require("./errors/formatFileCandidates"),
  InvalidPackageError: require("./errors/InvalidPackageError"),
  resolve: require("./resolve"),
};
module.exports = Resolver;
