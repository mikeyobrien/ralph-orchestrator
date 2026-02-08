const version = require("./package.json").version;

module.exports = {
	files: "./dist/*.js",
	from: /#__VERSION__#/g,
	to: version,
};
