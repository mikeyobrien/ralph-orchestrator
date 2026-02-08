# getenv

[![Build Status](https://secure.travis-ci.org/ctavan/node-getenv.png)](http://travis-ci.org/ctavan/node-getenv)

Helper to get and typecast environment variables. This is especially useful if you are building [Twelve-Factor-Apps](http://www.12factor.net/) where all configuration is stored in the environment.

## Installation

```
npm install getenv
```

TypeScript types are available from the `@types/getenv` module.

## Usage

Set environment variables:

```bash
export HTTP_HOST="localhost"
export HTTP_PORT=8080
export HTTP_START=true
export AB_TEST_RATIO=0.5
export KEYWORDS="sports,business"
export PRIMES="2,3,5,7"
```

Get and use them:

```javascript
const getenv = require('getenv');

const host = getenv('HTTP_HOST'); // same as getenv.string('HTTP_HOST');
const port = getenv.int('HTTP_PORT');
const start = getenv.bool('HTTP_START');

if (start === true) {
  // const server = http.createServer();
  // server.listen(port, host);
}

const abTestRatio = getenv.float('AB_TEST_RATIO');

if (Math.random() < abTestRatio) {
  // test A
} else {
  // test B
}

const keywords = getenv.array('KEYWORDS');
keywords.forEach(function (keyword) {
  // console.log(keyword);
});

const primes = getenv.array('PRIMES', 'int');
primes.forEach(function (prime) {
  // console.log(prime, typeof prime);
});
```

## Methods

All methods accept a fallback value that will be returned if the requested environment variable is not set. If the fallback value is omitted and if the requested environment variable does not exist, an exception is thrown.

### env(name, [fallback])

Alias for `env.string(name, [fallback])`.

### env.string(name, [fallback])

Return as string.

### env.int(name, [fallback])

Return as integer number.

### env.float(name, [fallback])

Return as float number.

### env.bool(name, [fallback])

Return as boolean. Only allows true/false as valid values.

### env.boolish(name, [fallback])

Return as boolean. Allows true/false/1/0 as valid values.

### env.array(name, [type], [fallback], [separator])

Split value of the environment variable at each comma (default) and return the resulting array where each value has been typecast according to the `type` parameter. An array can be provided as `fallback` and a regular expression can be provided as `separator` in case commas don't fit.

### env.multi({spec})

Return a list of environment variables based on a `spec`:

```javascript
const config = getenv.multi({
  foo: 'FOO', // throws if FOO doesn't exist
  bar: ['BAR', 'defaultval'], // set a default value
  baz: ['BAZ', 'defaultval', 'string'], // parse into type
  quux: ['QUUX', undefined, 'int'], // parse & throw
});
```

### env.url(name, [fallback])

Return a parsed URL as per Node's `require("url").parse`. N.B `url` doesn't validate URLs, so be sure it includes a protocol or you'll get deeply weird results.

```javascript
const serviceUrl = getenv.url('SERVICE_URL');

serviceUrl.port; // parsed port number
```

### env.disableFallbacks()

Disallows fallbacks in environments where you don't want to rely on brittle development defaults (e.g production, integration testing). For example, to disable fallbacks if we indicate production via `NODE_ENV`:

```javascript
if (process.env.NODE_ENV === 'production') {
  getenv.disableFallbacks();
}
```

### env.disableErrors()

`getenv` won't throw any error. If a fallback value is provided, that will be returned, else `undefined` is returned.

```javascript
getenv.disableErrors();
console.log(getenv('RANDOM'));
// undefined
```

### env.enableErrors()

Revert the effect of `disableErrors()`.

```javascript
getenv.disableErrors();
console.log(getenv('RANDOM'));
// undefined

getenv.enableErrors();
console.log(getenv('RANDOM'));
// Error: GetEnv.Nonexistent: RANDOM does not exist and no fallback value provided.
```

## Changelog

### v2.0.0

- Allow any case for True/False boolean strings `env.bool()` (#22)

### v1.1.0

- Add separator option to `env.array()` (#19)

### v1.0.0

- Drop support for Node.js older than 6.
- Modernize code.
- Add MIT License in package.json and LICENSE.md.

### v0.7.0

- Add env.disableErrors() / getenv.enableErrors() support.

### v0.6.0

- Added getenv.boolish() support.

### v0.5.0

- Add getenv.url() support.

### v0.4.0

- Add getenv.disableFallbacks() support.

### v0.3.0

- Add getenv.multi() support.

### v0.2.0

- Rename git repository

### v0.1.0

- Initial release

## Authors

- Moritz von Hase (initial author)
- Christoph Tavan <dev@tavan.de>
- Jonas Dohse <jonas@dohse.ch>
- Jan Lehnardt (@janl): `getenv.multi()` support.
- Tim Ruffles <timruffles@gmail.com>: `disableFallbacks()`, `url()`
- Ashwani Agarwal <ashwani.a@outlook.com>: `disableErrors()`, `enableErrors()`

## License

This module is licensed under the MIT license.
