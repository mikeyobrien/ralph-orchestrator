"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = validConfig;
async function validConfig() {
  const defaultConfig = await require("./index").default("/path/to/project");
  const validConfig = {
    ...defaultConfig,
    resolver: {
      ...defaultConfig.resolver,
      resolveRequest: function CustomResolver() {
        throw new Error("Not implemented");
      },
      hasteImplModulePath: "./path",
    },
    server: {
      ...defaultConfig.server,
      unstable_serverRoot: "",
    },
    transformer: {
      ...defaultConfig.transformer,
      getTransformOptions: function getTransformOptions() {
        throw new Error("Not implemented");
      },
    },
    serializer: {
      ...defaultConfig.serializer,
      customSerializer: function customSerializer() {
        throw new Error("Not implemented");
      },
    },
  };
  return validConfig;
}
