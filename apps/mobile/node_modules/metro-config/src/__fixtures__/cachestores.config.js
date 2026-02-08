"use strict";

module.exports = {
  cacheStores: ({ FileStore }) => {
    return [
      new FileStore({
        root: __dirname,
      }),
    ];
  },
};
