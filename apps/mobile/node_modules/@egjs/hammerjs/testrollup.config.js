import babel from "rollup-plugin-babel";
import { uglify } from "rollup-plugin-uglify";

export default [
	{
		input: "./tests/index.js",
		plugins: [babel({ exclude: "node_modules/**" })],
		output: {
			format: "umd",
			name: "Hammer",
			exports: "default",
			freeze: false,
			interop: false,
			sourcemap: true,
			file: "./testdist/hammer.js",
		},
	},
	{
		input: "./tests/index.js",
		plugins: [babel({ exclude: "node_modules/**" }), uglify({ sourcemap: true })],
		output: {
			format: "umd",
			name: "Hammer",
			exports: "default",
			freeze: false,
			interop: false,
			sourcemap: true,
			file: "./testdist/hammer.min.js",
		},
	},
];
