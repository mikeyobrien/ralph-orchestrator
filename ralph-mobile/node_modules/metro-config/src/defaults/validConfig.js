"use strict";

module.exports = async () => {
  const defaultConfig = await require("./index")("/path/to/project");
  const validConfig = {
    ...defaultConfig,
    resolver: {
      ...defaultConfig.resolver,
      resolveRequest: function CustomResolver() {},
      hasteImplModulePath: "./path",
    },
    server: {
      ...defaultConfig.server,
      unstable_serverRoot: "",
    },
    transformer: {
      ...defaultConfig.transformer,
      getTransformOptions: function getTransformOptions() {},
    },
    serializer: {
      ...defaultConfig.serializer,
      customSerializer: function customSerializer() {},
    },
  };
  return validConfig;
};
