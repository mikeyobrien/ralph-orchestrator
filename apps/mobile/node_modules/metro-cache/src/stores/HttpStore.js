"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _HttpError = _interopRequireDefault(require("./HttpError"));
var _NetworkError = _interopRequireDefault(require("./NetworkError"));
var _exponentialBackoff = require("exponential-backoff");
var _http = _interopRequireDefault(require("http"));
var _https = _interopRequireDefault(require("https"));
var _httpsProxyAgent = require("https-proxy-agent");
var _zlib = _interopRequireDefault(require("zlib"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const ZLIB_OPTIONS = {
  level: 9,
};
const NULL_BYTE = 0x00;
const NULL_BYTE_BUFFER = Buffer.from([NULL_BYTE]);
class HttpStore {
  static HttpError = _HttpError.default;
  static NetworkError = _NetworkError.default;
  #getEndpoint;
  #setEndpoint;
  constructor(options) {
    this.#getEndpoint = this.#createEndpointConfig(
      options.getOptions != null ? options.getOptions : options,
    );
    this.#setEndpoint = this.#createEndpointConfig(
      options.setOptions != null ? options.setOptions : options,
    );
  }
  #createEndpointConfig(options) {
    const agentConfig = {
      family: options.family,
      keepAlive: true,
      keepAliveMsecs: options.timeout || 5000,
      maxSockets: 64,
      maxFreeSockets: 64,
    };
    if (options.key != null) {
      agentConfig.key = options.key;
    }
    if (options.cert != null) {
      agentConfig.cert = options.cert;
    }
    if (options.ca != null) {
      agentConfig.ca = options.ca;
    }
    if (options.socketPath != null) {
      agentConfig.socketPath = options.socketPath;
    }
    const uri = new URL(options.endpoint);
    const module = uri.protocol === "http:" ? _http.default : _https.default;
    const agent =
      options.proxy != null
        ? new _httpsProxyAgent.HttpsProxyAgent(options.proxy, agentConfig)
        : new module.Agent(agentConfig);
    if (!uri.hostname || !uri.pathname) {
      throw new TypeError("Invalid endpoint: " + options.endpoint);
    }
    return {
      agent,
      headers: options.headers,
      host: uri.hostname,
      path: uri.pathname,
      port: +uri.port,
      params: new URLSearchParams(options.params),
      timeout: options.timeout || 5000,
      module: uri.protocol === "http:" ? _http.default : _https.default,
      additionalSuccessStatuses: new Set(
        options.additionalSuccessStatuses ?? [],
      ),
      debug: options.debug ?? false,
      maxAttempts: options.maxAttempts ?? 1,
      retryStatuses: new Set(options.retryStatuses ?? []),
      retryNetworkErrors: options.retryNetworkErrors ?? false,
    };
  }
  get(key) {
    return this.#withRetries(() => this.#getOnce(key), this.#getEndpoint);
  }
  #getOnce(key) {
    return new Promise((resolve, reject) => {
      let searchParamsString = this.#getEndpoint.params.toString();
      if (searchParamsString != "") {
        searchParamsString = "?" + searchParamsString;
      }
      const options = {
        agent: this.#getEndpoint.agent,
        headers: this.#getEndpoint.headers,
        host: this.#getEndpoint.host,
        method: "GET",
        path: `${this.#getEndpoint.path}/${key.toString("hex")}${searchParamsString}`,
        port: this.#getEndpoint.port,
        timeout: this.#getEndpoint.timeout,
      };
      const req = this.#getEndpoint.module.request(options, (res) => {
        const code = res.statusCode;
        const data = [];
        if (code === 404) {
          res.resume();
          resolve(null);
          return;
        } else if (
          code !== 200 &&
          !this.#getEndpoint.additionalSuccessStatuses.has(code)
        ) {
          if (this.#getEndpoint.debug) {
            res.on("data", (chunk) => {
              data.push(chunk);
            });
            res.on("error", (err) => {
              reject(
                new _HttpError.default(
                  "Encountered network error (" +
                    err.message +
                    ") while handling HTTP error: " +
                    code +
                    " " +
                    _http.default.STATUS_CODES[code],
                  code,
                ),
              );
            });
            res.on("end", () => {
              const buffer = Buffer.concat(data);
              reject(
                new _HttpError.default(
                  "HTTP error: " +
                    code +
                    " " +
                    _http.default.STATUS_CODES[code] +
                    "\n\n" +
                    buffer.toString(),
                  code,
                ),
              );
            });
          } else {
            res.resume();
            reject(
              new _HttpError.default(
                "HTTP error: " + code + " " + _http.default.STATUS_CODES[code],
                code,
              ),
            );
          }
          return;
        }
        const gunzipped = res.pipe(_zlib.default.createGunzip());
        gunzipped.on("data", (chunk) => {
          data.push(chunk);
        });
        gunzipped.on("error", (err) => {
          reject(err);
        });
        gunzipped.on("end", () => {
          try {
            const buffer = Buffer.concat(data);
            if (buffer.length > 0 && buffer[0] === NULL_BYTE) {
              resolve(buffer.slice(1));
            } else {
              resolve(JSON.parse(buffer.toString("utf8")));
            }
          } catch (err) {
            reject(err);
          }
        });
        res.on("error", (err) => gunzipped.emit("error", err));
      });
      req.on("error", (err) => {
        reject(new _NetworkError.default(err.message, err.code));
      });
      req.on("timeout", () => {
        req.destroy(new Error("Request timed out"));
      });
      req.end();
    });
  }
  set(key, value) {
    return this.#withRetries(
      () => this.#setOnce(key, value),
      this.#setEndpoint,
    );
  }
  #setOnce(key, value) {
    return new Promise((resolve, reject) => {
      const gzip = _zlib.default.createGzip(ZLIB_OPTIONS);
      let searchParamsString = this.#setEndpoint.params.toString();
      if (searchParamsString != "") {
        searchParamsString = "?" + searchParamsString;
      }
      const options = {
        agent: this.#setEndpoint.agent,
        headers: this.#setEndpoint.headers,
        host: this.#setEndpoint.host,
        method: "PUT",
        path: `${this.#setEndpoint.path}/${key.toString("hex")}${searchParamsString}`,
        port: this.#setEndpoint.port,
        timeout: this.#setEndpoint.timeout,
      };
      const req = this.#setEndpoint.module.request(options, (res) => {
        const code = res.statusCode;
        if (
          (code < 200 || code > 299) &&
          !this.#setEndpoint.additionalSuccessStatuses.has(code)
        ) {
          if (this.#setEndpoint.debug) {
            const data = [];
            res.on("data", (chunk) => {
              data.push(chunk);
            });
            res.on("error", (err) => {
              reject(
                new _HttpError.default(
                  "Encountered network error (" +
                    err.message +
                    ") while handling HTTP error: " +
                    code +
                    " " +
                    _http.default.STATUS_CODES[code],
                  code,
                ),
              );
            });
            res.on("end", () => {
              const buffer = Buffer.concat(data);
              reject(
                new _HttpError.default(
                  "HTTP error: " +
                    code +
                    " " +
                    _http.default.STATUS_CODES[code] +
                    "\n\n" +
                    buffer.toString(),
                  code,
                ),
              );
            });
          } else {
            res.resume();
            reject(
              new _HttpError.default(
                "HTTP error: " + code + " " + _http.default.STATUS_CODES[code],
                code,
              ),
            );
          }
          return;
        }
        res.on("error", (err) => {
          reject(err);
        });
        res.on("end", () => {
          resolve();
        });
        res.resume();
      });
      req.on("timeout", () => {
        req.destroy(new Error("Request timed out"));
      });
      gzip.pipe(req);
      if (value instanceof Buffer) {
        gzip.write(NULL_BYTE_BUFFER);
        gzip.end(value);
      } else {
        gzip.end(JSON.stringify(value) || "null");
      }
    });
  }
  clear() {}
  #withRetries(fn, endpoint) {
    if (endpoint.maxAttempts === 1) {
      return fn();
    }
    return (0, _exponentialBackoff.backOff)(fn, {
      jitter: "full",
      maxDelay: 30000,
      numOfAttempts: this.#getEndpoint.maxAttempts || Number.POSITIVE_INFINITY,
      retry: (e) => {
        if (e instanceof _HttpError.default) {
          return this.#getEndpoint.retryStatuses.has(e.code);
        }
        return (
          e instanceof _NetworkError.default &&
          this.#getEndpoint.retryNetworkErrors
        );
      },
    });
  }
}
exports.default = HttpStore;
