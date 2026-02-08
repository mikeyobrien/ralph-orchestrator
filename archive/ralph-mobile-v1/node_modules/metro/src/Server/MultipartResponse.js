"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _accepts = _interopRequireDefault(require("accepts"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const CRLF = "\r\n";
const BOUNDARY = "3beqjf3apnqeu3h5jqorms4i";
class MultipartResponse {
  static wrapIfSupported(req, res) {
    if ((0, _accepts.default)(req).types().includes("multipart/mixed")) {
      return new MultipartResponse(res);
    }
    return res;
  }
  static serializeHeaders(headers) {
    return Object.keys(headers)
      .map((key) => `${key}: ${headers[key]}`)
      .join(CRLF);
  }
  constructor(res) {
    this.res = res;
    this.headers = {};
    res.writeHead(200, {
      "Content-Type": `multipart/mixed; boundary="${BOUNDARY}"`,
    });
    res.write(
      "If you are seeing this, your client does not support multipart response",
    );
  }
  writeChunk(headers, data, isLast = false) {
    if (this.res.finished) {
      return;
    }
    this.res.write(`${CRLF}--${BOUNDARY}${CRLF}`);
    if (headers) {
      this.res.write(MultipartResponse.serializeHeaders(headers) + CRLF + CRLF);
    }
    if (data != null) {
      this.res.write(data);
    }
    if (isLast) {
      this.res.write(`${CRLF}--${BOUNDARY}--${CRLF}`);
    }
  }
  writeHead(status, headers) {
    this.setHeader("X-Http-Status", status);
    if (!headers) {
      return;
    }
    for (const key in headers) {
      this.setHeader(key, headers[key]);
    }
  }
  setHeader(name, value) {
    this.headers[name] = value;
  }
  end(data) {
    this.writeChunk(this.headers, data, true);
    this.res.end();
  }
  once(name, fn) {
    this.res.once(name, fn);
    return this;
  }
}
exports.default = MultipartResponse;
