var pkg = require("./package.json");
var date = new Date();

module.exports = `/*! ${pkg.title || pkg.name} - v${pkg.version} - ${[ date.getFullYear(), ('0' + (date.getMonth() + 1)).slice(-2), ('0' + date.getDate()).slice(-2)].join('-')}
 * ${pkg.homepage}
 *
 * Forked By Naver egjs
 * Copyright (c) hammerjs
 * Licensed under the ${pkg.license} license */`;