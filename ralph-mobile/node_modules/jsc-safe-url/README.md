## `jsc-safe-url`

JavaScriptCore santizes source URLs in error stacks by stripping query strings and fragments. Ref: [`Webkit/Webkit#49109d`](https://github.com/WebKit/WebKit/commit/49109db4ab87a715f7a8987c7ee380e63060298b).

This package contains utility functions required to implement the proposed [React Native Community RFC0646](https://github.com/react-native-community/discussions-and-proposals/pull/646). It exports three functons:

## `function isJscSafeUrl(url: string): boolean`

Returns `false` if the URL has a query component that could be stripped by JSC.

## `function toJscSafeUrl(urlToConvert: string): string`

Accepts an absolute or relative URL, and encodes any data in the input query string (if present) into the [path component](https://www.rfc-editor.org/rfc/rfc3986#section-3.3) of the URL, by using the delimiter `//&` (which cannot appear in a normalized file path) to separate the original path from the orignal query string.

```
toJscSafeUrl('https://example.com/path?foo=bar#fragment')
// 'https://example.com/path//&foo=bar#fragment'
```

## `function toNormalUrl(urlToNormalize: string): string`

Accepts an absolute or relative URL, and replaces the first unescaped `//&` in the [path component](https://www.rfc-editor.org/rfc/rfc3986#section-3.3) with `?`. (Effectively the reverse of `toJscSafeUrl`.)

```
toNormalUrl('https://example.com/path//&foo=bar#fragment')
// 'https://example.com/path?foo=bar#fragment'
```
