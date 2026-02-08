"use strict";

const HttpError = require("./HttpError");
const NetworkError = require("./NetworkError");
const { backOff } = require("exponential-backoff");
const http = require("http");
const https = require("https");
const url = require("url");
const zlib = require("zlib");
const ZLIB_OPTIONS = {
  level: 9,
};
const NULL_BYTE = 0x00;
const NULL_BYTE_BUFFER = Buffer.from([NULL_BYTE]);
class HttpStore {
  static HttpError = HttpError;
  static NetworkError = NetworkError;
  constructor(options) {
    this._getEndpoint = this.createEndpointConfig(
      options.getOptions != null ? options.getOptions : options
    );
    this._setEndpoint = this.createEndpointConfig(
      options.setOptions != null ? options.setOptions : options
    );
  }
  createEndpointConfig(options) {
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
    const uri = url.parse(options.endpoint);
    const module = uri.protocol === "http:" ? http : https;
    if (!uri.hostname || !uri.pathname) {
      throw new TypeError("Invalid endpoint: " + options.endpoint);
    }
    return {
      headers: options.headers,
      host: uri.hostname,
      path: uri.pathname,
      port: +uri.port,
      agent: new module.Agent(agentConfig),
      params: new URLSearchParams(options.params),
      timeout: options.timeout || 5000,
      module: uri.protocol === "http:" ? http : https,
      additionalSuccessStatuses: new Set(
        options.additionalSuccessStatuses ?? []
      ),
      debug: options.debug ?? false,
      maxAttempts: options.maxAttempts ?? 1,
      retryStatuses: new Set(options.retryStatuses ?? []),
      retryNetworkErrors: options.retryNetworkErrors ?? false,
    };
  }
  get(key) {
    return this.#withRetries(() => this.#getOnce(key), this._getEndpoint);
  }
  #getOnce(key) {
    return new Promise((resolve, reject) => {
      let searchParamsString = this._getEndpoint.params.toString();
      if (searchParamsString != "") {
        searchParamsString = "?" + searchParamsString;
      }
      const options = {
        agent: this._getEndpoint.agent,
        headers: this._getEndpoint.headers,
        host: this._getEndpoint.host,
        method: "GET",
        path: `${this._getEndpoint.path}/${key.toString(
          "hex"
        )}${searchParamsString}`,
        port: this._getEndpoint.port,
        timeout: this._getEndpoint.timeout,
      };
      const req = this._getEndpoint.module.request(options, (res) => {
        const code = res.statusCode;
        const data = [];
        if (code === 404) {
          res.resume();
          resolve(null);
          return;
        } else if (
          code !== 200 &&
          !this._getEndpoint.additionalSuccessStatuses.has(code)
        ) {
          if (this._getEndpoint.debug) {
            res.on("data", (chunk) => {
              data.push(chunk);
            });
            res.on("error", (err) => {
              reject(
                new HttpError(
                  "Encountered network error (" +
                    err.message +
                    ") while handling HTTP error: " +
                    code +
                    " " +
                    http.STATUS_CODES[code],
                  code
                )
              );
            });
            res.on("end", () => {
              const buffer = Buffer.concat(data);
              reject(
                new HttpError(
                  "HTTP error: " +
                    code +
                    " " +
                    http.STATUS_CODES[code] +
                    "\n\n" +
                    buffer.toString(),
                  code
                )
              );
            });
          } else {
            res.resume();
            reject(
              new HttpError(
                "HTTP error: " + code + " " + http.STATUS_CODES[code],
                code
              )
            );
          }
          return;
        }
        const gunzipped = res.pipe(zlib.createGunzip());
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
        reject(new NetworkError(err.message, err.code));
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
      this._setEndpoint
    );
  }
  #setOnce(key, value) {
    return new Promise((resolve, reject) => {
      const gzip = zlib.createGzip(ZLIB_OPTIONS);
      let searchParamsString = this._setEndpoint.params.toString();
      if (searchParamsString != "") {
        searchParamsString = "?" + searchParamsString;
      }
      const options = {
        agent: this._setEndpoint.agent,
        headers: this._setEndpoint.headers,
        host: this._setEndpoint.host,
        method: "PUT",
        path: `${this._setEndpoint.path}/${key.toString(
          "hex"
        )}${searchParamsString}`,
        port: this._setEndpoint.port,
        timeout: this._setEndpoint.timeout,
      };
      const req = this._setEndpoint.module.request(options, (res) => {
        const code = res.statusCode;
        if (
          (code < 200 || code > 299) &&
          !this._setEndpoint.additionalSuccessStatuses.has(code)
        ) {
          if (this._setEndpoint.debug) {
            const data = [];
            res.on("data", (chunk) => {
              data.push(chunk);
            });
            res.on("error", (err) => {
              reject(
                new HttpError(
                  "Encountered network error (" +
                    err.message +
                    ") while handling HTTP error: " +
                    code +
                    " " +
                    http.STATUS_CODES[code],
                  code
                )
              );
            });
            res.on("end", () => {
              const buffer = Buffer.concat(data);
              reject(
                new HttpError(
                  "HTTP error: " +
                    code +
                    " " +
                    http.STATUS_CODES[code] +
                    "\n\n" +
                    buffer.toString(),
                  code
                )
              );
            });
          } else {
            res.resume();
            reject(
              new HttpError(
                "HTTP error: " + code + " " + http.STATUS_CODES[code],
                code
              )
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
    return backOff(fn, {
      jitter: "full",
      maxDelay: 30000,
      numOfAttempts: this._getEndpoint.maxAttempts || Number.POSITIVE_INFINITY,
      retry: (e) => {
        if (e instanceof HttpError) {
          return this._getEndpoint.retryStatuses.has(e.code);
        }
        return (
          e instanceof NetworkError && this._getEndpoint.retryNetworkErrors
        );
      },
    });
  }
}
module.exports = HttpStore;
