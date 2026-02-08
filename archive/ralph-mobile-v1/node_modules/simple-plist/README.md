# `simple-plist`

[![npm](https://img.shields.io/npm/dw/simple-plist.svg?style=popout&logo=npm)](https://www.npmjs.org/package/simple-plist)
[![npm](https://img.shields.io/npm/v/simple-plist.svg?style=popout&logo=npm)](https://www.npmjs.com/package/simple-plist)

A simple API for interacting with binary and plain text
[plist](https://en.wikipedia.org/wiki/Property_list) data.

## Installation

```sh
# via npm
npm install simple-plist

# via yarn
yarn add simple-plist
```

## Synchronous API

```js
const plist = require("simple-plist");

let data;

// read
data = plist.readFileSync("/path/to/some.plist");

// write xml
plist.writeFileSync("/path/to/plaintext.plist", data);

// write binary
plist.writeBinaryFileSync("/path/to/binary.plist", data);
```

## Asynchronous API

> Note: all of the async examples can optionally be converted to promises using
> node's [`util.promisify`](https://nodejs.org/dist/latest-v8.x/docs/api/util.html#util_util_promisify_original).

```js
const plist = require("simple-plist");

let data;

function callback(err, contents) {
  if (err) throw err;
  data = contents;
}

// read
plist.readFile("/path/to/some.plist", callback);

// write xml
plist.writeFile("/path/to/plaintext.plist", data, callback);

// write binary
plist.writeBinaryFile("/path/to/binary.plist", data, callback);
```

## In Memory

### `plist.stringify()`

```js
const plist = require("simple-plist");

// Convert an object to a plist xml string
plist.stringify({ name: "Joe", answer: 42 });

/*
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>name</key>
    <string>Joe</string>
    <key>answer</key>
    <integer>42</integer>
  </dict>
</plist>
*/
```

### `plist.parse()`

```js
const plist = require("simple-plist");

const xml = `<plist>
	<dict>
		<key>name</key>
		<string>Joe</string>
	</dict>
</plist>`;

plist.parse(xml);
// { "name": "Joe" }
```

## TypeScript Support

All functions have typescript signatures, but there are a few handy generics
that are worth pointing out. Those generics belong to `parse`, `readFile`,
and `readFileSync`. Here's an example:

```tsx
import { parse, readFile, readFileSync } from "simple-plist";

type Profile = {
  name: string;
  answer: number;
};

const xml = `<plist>
	<dict>
		<key>name</key>
		<string>Joe</string>
		<key>answer</key>
		<integer>42</integer>
	</dict>
</plist>`;

// typed string parsing
const { answer } = parse<Profile>(xml);
// answer = 42;

// typed file loading
const { name } = readFileSync<Profile>("/path/to/profile.plist");
```
