"use strict";

const defaultDependencyExtractor = require("./lib/dependencyExtractor");
const excludedExtensions = require("./workerExclusionList");
const { createHash } = require("crypto");
const fs = require("graceful-fs");
const path = require("path");
const PACKAGE_JSON = path.sep + "package.json";
let hasteImpl = null;
let hasteImplModulePath = null;
function getHasteImpl(requestedModulePath) {
  if (hasteImpl) {
    if (requestedModulePath !== hasteImplModulePath) {
      throw new Error("metro-file-map: hasteImplModulePath changed");
    }
    return hasteImpl;
  }
  hasteImplModulePath = requestedModulePath;
  hasteImpl = require(hasteImplModulePath);
  return hasteImpl;
}
function sha1hex(content) {
  return createHash("sha1").update(content).digest("hex");
}
class Worker {
  constructor(args) {}
  processFile(data) {
    let content;
    let dependencies;
    let id;
    let sha1;
    const { computeDependencies, computeSha1, enableHastePackages, filePath } =
      data;
    const getContent = () => {
      if (content == null) {
        content = fs.readFileSync(filePath);
      }
      return content;
    };
    if (enableHastePackages && filePath.endsWith(PACKAGE_JSON)) {
      try {
        const fileData = JSON.parse(getContent().toString());
        if (fileData.name) {
          id = fileData.name;
        }
      } catch (err) {
        throw new Error(`Cannot parse ${filePath} as JSON: ${err.message}`);
      }
    } else if (
      (data.hasteImplModulePath != null || computeDependencies) &&
      !excludedExtensions.has(filePath.substr(filePath.lastIndexOf(".")))
    ) {
      if (data.hasteImplModulePath != null) {
        id = getHasteImpl(data.hasteImplModulePath).getHasteName(filePath);
      }
      if (computeDependencies) {
        const dependencyExtractor =
          data.dependencyExtractor != null
            ? require(data.dependencyExtractor)
            : null;
        dependencies = Array.from(
          dependencyExtractor != null
            ? dependencyExtractor.extract(
                getContent().toString(),
                filePath,
                defaultDependencyExtractor.extract,
              )
            : defaultDependencyExtractor.extract(getContent().toString()),
        );
      }
    }
    if (computeSha1) {
      sha1 = sha1hex(getContent());
    }
    return content && data.maybeReturnContent
      ? {
          content,
          dependencies,
          id,
          sha1,
        }
      : {
          dependencies,
          id,
          sha1,
        };
  }
}
let singletonWorker;
function setup(args) {
  if (singletonWorker) {
    throw new Error("metro-file-map: setup() should only be called once");
  }
  singletonWorker = new Worker(args);
}
function processFile(data) {
  if (!singletonWorker) {
    throw new Error(
      "metro-file-map: setup() must be called before processFile()",
    );
  }
  return singletonWorker.processFile(data);
}
module.exports = {
  setup,
  processFile,
  Worker,
};
