/// <reference types="node"/>
import {Metadata, PNG, PNGOptions} from 'pngjs';

/**
Parse a PNG.

@param buffer - A PNG image buffer.
@param options - See the [pngjs options](https://github.com/lukeapage/pngjs#options).
@returns The parsed PNG image.

@example
```
import * as fs from 'fs';
import parsePng = require('parse-png');

(async () => {
	const png = await parsePng(fs.readFileSync('unicorn.png'));

	png.adjustGamma();
	png.pack().pipe(fs.createWriteStream('unicorn-adjusted.png'));
})();
```
*/
declare function parsePng(
	buffer: Buffer,
	options?: PNGOptions
): Promise<PNG & Metadata>;

export = parsePng;
