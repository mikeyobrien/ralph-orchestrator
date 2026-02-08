"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = void 0;
var _fs = _interopRequireDefault(require("fs"));
var _path = _interopRequireDefault(require("path"));
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
const NULL_BYTE = 0x00;
const NULL_BYTE_BUFFER = Buffer.from([NULL_BYTE]);
class FileStore {
  #root;
  constructor(options) {
    this.#root = options.root;
  }
  async get(key) {
    try {
      const data = await _fs.default.promises.readFile(this.#getFilePath(key));
      if (data[0] === NULL_BYTE) {
        return data.slice(1);
      }
      return JSON.parse(data.toString("utf8"));
    } catch (err) {
      if (err.code === "ENOENT" || err instanceof SyntaxError) {
        return null;
      }
      throw err;
    }
  }
  async set(key, value) {
    const filePath = this.#getFilePath(key);
    try {
      await this.#set(filePath, value);
    } catch (err) {
      if (err.code === "ENOENT") {
        _fs.default.mkdirSync(_path.default.dirname(filePath), {
          recursive: true,
        });
        await this.#set(filePath, value);
      } else {
        throw err;
      }
    }
  }
  async #set(filePath, value) {
    let content;
    if (value instanceof Buffer) {
      content = Buffer.concat([NULL_BYTE_BUFFER, value]);
    } else {
      content = JSON.stringify(value) ?? JSON.stringify(null);
    }
    await _fs.default.promises.writeFile(filePath, content);
  }
  clear() {
    this.#removeDirs();
  }
  #getFilePath(key) {
    return _path.default.join(
      this.#root,
      key.slice(0, 1).toString("hex"),
      key.slice(1).toString("hex"),
    );
  }
  #removeDirs() {
    for (let i = 0; i < 256; i++) {
      _fs.default.rmSync(
        _path.default.join(this.#root, ("0" + i.toString(16)).slice(-2)),
        {
          force: true,
          recursive: true,
        },
      );
    }
  }
}
exports.default = FileStore;
