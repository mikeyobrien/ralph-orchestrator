import babel from "rollup-plugin-babel";
import { uglify } from "rollup-plugin-uglify";
import replace from "rollup-plugin-replace";

const banner = require("./banner");
const version = require("./package.json").version;
const replaceVersion = replace({"#__VERSION__#": version, delimiters: ["", ""]});

export default [
	{
		input: "src/index.js",
		plugins: [babel({ exclude: "node_modules/**" }), replaceVersion],
		output: {
			banner: banner,
			format: "es",
			freeze: false,
			exprots: "named",
			interop: false,
			sourcemap: true,
			file: "./dist/hammer.esm.js",
		},
	},
	{
		input: "src/hammer.js",
		plugins: [babel({ exclude: "node_modules/**" }), replaceVersion],
		output: {
			banner: banner,
			format: "umd",
			name: "Hammer",
			exports: "default",
			freeze: false,
			interop: false,
			sourcemap: true,
			file: "./dist/hammer.js",
		},
	},
	{
		input: "src/hammer.js",
		plugins: [babel({ exclude: "node_modules/**" }), replaceVersion, uglify({
			sourcemap: true,
			output: {
				comments: function (node, comment) {
					const text = comment.value;
					const type = comment.type;

					if (type === "comment2") {
						// multiline comment
						return /Naver/.test(text);
					}
					return false;
				},
			},
		})],
		output: {
			banner: banner,
			format: "umd",
			name: "Hammer",
			exports: "default",
			freeze: false,
			interop: false,
			sourcemap: true,
			file: "./dist/hammer.min.js",
		},
	},
];
