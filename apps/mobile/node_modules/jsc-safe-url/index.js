/**
 * These functions are for handling of query-string free URLs, necessitated
 * by query string stripping of URLs in JavaScriptCore stack traces
 * introduced in iOS 16.4.
 *
 * See https://github.com/facebook/react-native/issues/36794 for context.
 *
 * @flow strict
 */

// We use regex-based URL parsing as defined in RFC3986 because it's easier to
// determine whether the input is a complete URI, a path-absolute or a
// path-rootless (as defined in the spec), and be as faithful to the input as
// possible. This will match any string, and does not imply validity.
//
// https://www.rfc-editor.org/rfc/rfc3986#appendix-B
const URI_REGEX = /^(([^:/?#]+):)?(\/\/([^/?#]*))?([^?#]*)(\?([^#]*))?(#(.*))?/;

function _rfc3986Parse(url /*:string */) {
  const match = url.match(URI_REGEX);
  if (match == null) {
    throw new Error("Unexpected error - failed to regex-match URL");
  }
  return {
    schemeAndAuthority: (match[1] || "") + (match[3] || ""),
    path: match[5] || "",
    hasQueryPart: match[6] != null,
    queryWithoutQuestionMark: match[7] || "",
    fragmentWithHash: match[8] || "",
  };
}

function isJscSafeUrl(url /*:string */) /*:boolean*/ {
  const parsedUrl = _rfc3986Parse(url);
  return !parsedUrl.hasQueryPart;
}

/**
 * @param {string} urlToNormalize
 * @returns string
 */
function toNormalUrl(urlToNormalize /*:string */) /*:string */ {
  const parsedUrl = _rfc3986Parse(urlToNormalize);
  if (parsedUrl.path.indexOf("//&") === -1) {
    return urlToNormalize;
  }
  return (
    parsedUrl.schemeAndAuthority +
    parsedUrl.path.replace("//&", "?") +
    // We don't expect JSC urls to also have query strings, but interpret
    // liberally and append them.
    (parsedUrl.queryWithoutQuestionMark.length > 0
      ? "&" + parsedUrl.queryWithoutQuestionMark
      : "") +
    // Likewise, JSC URLs will usually have their fragments stripped, but
    // preserve if we find one.
    parsedUrl.fragmentWithHash
  );
}

/**
 * @param {string} urlToConvert
 * @returns string
 */
function toJscSafeUrl(urlToConvert /*:string */) /*:string */ {
  if (!_rfc3986Parse(urlToConvert).hasQueryPart) {
    return urlToConvert;
  }
  const parsedUrl = _rfc3986Parse(toNormalUrl(urlToConvert));
  if (
    parsedUrl.queryWithoutQuestionMark.length > 0 &&
    (parsedUrl.path === "" || parsedUrl.path === "/")
  ) {
    throw new Error(
      `The given URL "${urlToConvert}" has an empty path and cannot be converted to a JSC-safe format.`
    );
  }
  return (
    parsedUrl.schemeAndAuthority +
    parsedUrl.path +
    (parsedUrl.queryWithoutQuestionMark.length > 0
      ? "//&" +
        // Query strings may contain '?' (e.g. in key or value names) - these
        // must be percent-encoded to form a valid path, and not be stripped.
        parsedUrl.queryWithoutQuestionMark.replace(/\?/g, "%3F")
      : "") +
    // We expect JSC to strip this - we don't handle fragments for now.
    parsedUrl.fragmentWithHash
  );
}

module.exports = {
  isJscSafeUrl,
  toNormalUrl,
  toJscSafeUrl,
};
