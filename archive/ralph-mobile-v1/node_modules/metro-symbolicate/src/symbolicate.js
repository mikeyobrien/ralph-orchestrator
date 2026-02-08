"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true,
});
exports.default = main;
var Symbolication = _interopRequireWildcard(require("./Symbolication"));
var _fs = _interopRequireDefault(require("fs"));
var _sourceMap = require("source-map");
var _stream = require("stream");
function _interopRequireDefault(e) {
  return e && e.__esModule ? e : { default: e };
}
function _getRequireWildcardCache(e) {
  if ("function" != typeof WeakMap) return null;
  var r = new WeakMap(),
    t = new WeakMap();
  return (_getRequireWildcardCache = function (e) {
    return e ? t : r;
  })(e);
}
function _interopRequireWildcard(e, r) {
  if (!r && e && e.__esModule) return e;
  if (null === e || ("object" != typeof e && "function" != typeof e))
    return { default: e };
  var t = _getRequireWildcardCache(r);
  if (t && t.has(e)) return t.get(e);
  var n = { __proto__: null },
    a = Object.defineProperty && Object.getOwnPropertyDescriptor;
  for (var u in e)
    if ("default" !== u && {}.hasOwnProperty.call(e, u)) {
      var i = a ? Object.getOwnPropertyDescriptor(e, u) : null;
      i && (i.get || i.set) ? Object.defineProperty(n, u, i) : (n[u] = e[u]);
    }
  return ((n.default = e), t && t.set(e, n), n);
}
function printHelp() {
  const usages = [
    "Usage: " + __filename + " <source-map-file>",
    "       " + __filename + " <source-map-file> <line> [column]",
    "       " + __filename + " <source-map-file> <moduleId>.js <line> [column]",
    "       " + __filename + " <source-map-file> <mapfile>.profmap",
    "       " +
      __filename +
      " <source-map-file> --attribution < in.jsonl > out.jsonl",
    "       " + __filename + " <source-map-file> <tracefile>.cpuprofile",
    " Optional flags:",
    "  --no-function-names",
    "  --hermes-crash (mutually exclusive with --hermes-coverage)",
    "  --hermes-coverage (mutually exclusive with --hermes-crash)",
    "  --input-line-start <line> (default: 1)",
    "  --input-column-start <column> (default: 0)",
    "  --output-line-start <line> (default: 1)",
    "  --output-column-start <column> (default: 0)",
  ];
  console.error(usages.join("\n"));
}
async function main(
  argvInput = process.argv.slice(2),
  { stdin, stderr, stdout } = process,
) {
  const argv = argvInput.slice();
  function checkAndRemoveArg(arg, valuesPerArg = 0) {
    let values = null;
    for (let idx = argv.indexOf(arg); idx !== -1; idx = argv.indexOf(arg)) {
      argv.splice(idx, 1);
      values = values || [];
      values.push(argv.splice(idx, valuesPerArg));
    }
    return values;
  }
  function checkAndRemoveArgWithValue(arg) {
    const values = checkAndRemoveArg(arg, 1);
    return values ? values[0][0] : null;
  }
  try {
    const noFunctionNames = checkAndRemoveArg("--no-function-names");
    const isHermesCrash = checkAndRemoveArg("--hermes-crash");
    const isCoverage = checkAndRemoveArg("--hermes-coverage");
    const inputLineStart = Number.parseInt(
      checkAndRemoveArgWithValue("--input-line-start") || "1",
      10,
    );
    const inputColumnStart = Number.parseInt(
      checkAndRemoveArgWithValue("--input-column-start") || "0",
      10,
    );
    const outputLineStart = Number.parseInt(
      checkAndRemoveArgWithValue("--output-line-start") || "1",
      10,
    );
    const outputColumnStart = Number.parseInt(
      checkAndRemoveArgWithValue("--output-column-start") || "0",
      10,
    );
    if (argv.length < 1 || argv.length > 4) {
      printHelp();
      return 1;
    }
    if (isHermesCrash && isCoverage) {
      console.error(
        "Pass either --hermes-crash or --hermes-coverage, not both",
      );
      printHelp();
      return 1;
    }
    const sourceMapFileName = argv.shift();
    const options = {
      nameSource: noFunctionNames ? "identifier_names" : "function_names",
      inputLineStart,
      inputColumnStart,
      outputLineStart,
      outputColumnStart,
    };
    let context;
    if (_fs.default.lstatSync(sourceMapFileName).isDirectory()) {
      context = Symbolication.unstable_createDirectoryContext(
        _sourceMap.SourceMapConsumer,
        sourceMapFileName,
        options,
      );
    } else {
      const content = _fs.default.readFileSync(sourceMapFileName, "utf8");
      context = Symbolication.createContext(
        _sourceMap.SourceMapConsumer,
        content,
        options,
      );
    }
    if (argv.length === 0) {
      const stackTrace = await readAll(stdin);
      if (isHermesCrash) {
        const stackTraceJSON = JSON.parse(stackTrace);
        const symbolicatedTrace =
          context.symbolicateHermesMinidumpTrace(stackTraceJSON);
        stdout.write(JSON.stringify(symbolicatedTrace));
      } else if (isCoverage) {
        const stackTraceJSON = JSON.parse(stackTrace);
        const symbolicatedTrace =
          context.symbolicateHermesCoverageTrace(stackTraceJSON);
        stdout.write(JSON.stringify(symbolicatedTrace));
      } else {
        stdout.write(context.symbolicate(stackTrace));
      }
    } else if (argv[0].endsWith(".profmap")) {
      stdout.write(context.symbolicateProfilerMap(argv[0]));
    } else if (
      argv[0].endsWith(".heapsnapshot") ||
      argv[0].endsWith(".heaptimeline")
    ) {
      stdout.write(
        JSON.stringify(
          context.symbolicateHeapSnapshot(
            _fs.default.readFileSync(argv[0], "utf8"),
          ),
        ),
      );
    } else if (argv[0] === "--attribution") {
      let lineBuffer = "";
      const streamToLines = new _stream.Transform({
        transform(data, _enc, callback) {
          lineBuffer += data.toString();
          const lines = lineBuffer.split("\n");
          for (let i = 0, e = lines.length - 1; i < e; i++) {
            streamToLines.push(lines[i]);
          }
          lineBuffer = lines[lines.length - 1];
          callback();
        },
      });
      const symbolicateLines = new _stream.Transform({
        transform(data, enc, callback) {
          const obj = JSON.parse(data.toString());
          context.symbolicateAttribution(obj);
          symbolicateLines.push(JSON.stringify(obj) + "\n");
          callback();
        },
        objectMode: true,
      });
      await waitForStream(
        stdin.pipe(streamToLines).pipe(symbolicateLines).pipe(stdout),
      );
    } else if (argv[0].endsWith(".cpuprofile")) {
      context.symbolicateChromeTrace(argv[0], {
        stdout,
        stderr,
      });
    } else {
      let moduleIds;
      if (argv[0].endsWith(".js")) {
        moduleIds = context.parseFileName(argv[0]);
        argv.shift();
      } else {
        moduleIds = null;
      }
      const lineNumber = argv.shift();
      const columnNumber = argv.shift() || 0;
      const original = context.getOriginalPositionFor(
        +lineNumber,
        +columnNumber,
        moduleIds,
      );
      stdout.write(
        [
          original.source ?? "null",
          original.line ?? "null",
          original.name ?? "null",
        ].join(":") + "\n",
      );
    }
  } catch (error) {
    stderr.write(error + "\n");
    return 1;
  }
  return 0;
}
function readAll(stream) {
  return new Promise((resolve) => {
    let data = "";
    if (stream.isTTY === true) {
      resolve(data);
      return;
    }
    stream.setEncoding("utf8");
    stream.on("readable", () => {
      let chunk;
      while ((chunk = stream.read())) {
        data += chunk.toString();
      }
    });
    stream.on("end", () => {
      resolve(data);
    });
  });
}
function waitForStream(stream) {
  return new Promise((resolve) => {
    stream.on("finish", resolve);
  });
}
