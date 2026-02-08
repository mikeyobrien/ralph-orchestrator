'use strict';
const {PNG} = require('pngjs');

module.exports = async (buffer, options) => {
	if (!Buffer.isBuffer(buffer)) {
		throw new TypeError(`Expected \`buffer\` to be of type \`Buffer\` but received type \`${typeof buffer}\``);
	}

	return new Promise((resolve, reject) => {
		let png = new PNG(options);

		png.on('metadata', data => {
			png = Object.assign(png, data);
		});

		png.on('error', reject);
		png.on('parsed', () => resolve(png));

		png.end(buffer);
	});
};
