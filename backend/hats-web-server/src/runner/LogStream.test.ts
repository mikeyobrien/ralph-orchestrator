import { test } from "node:test";
import assert from "node:assert";
import { LogStream } from "./LogStream";

test("LogStream strips ANSI codes from output", () => {
  const stream = new LogStream();
  const ansiString = "\u001b[31mRed Text\u001b[39m";

  stream.writeStdout(ansiString + "\n");

  const entries = stream.getStdout();
  assert.strictEqual(entries.length, 1);
  assert.strictEqual(entries[0].line, "Red Text");
});

test("LogStream strips ANSI codes from flushed buffer", () => {
  const stream = new LogStream();
  const ansiString = "\u001b[32mGreen Text\u001b[39m";

  stream.writeStdout(ansiString);
  stream.flush();

  const entries = stream.getStdout();
  assert.strictEqual(entries.length, 1);
  assert.strictEqual(entries[0].line, "Green Text");
});
