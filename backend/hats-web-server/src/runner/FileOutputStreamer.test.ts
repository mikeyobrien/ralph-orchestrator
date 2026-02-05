/**
 * Unit tests for FileOutputStreamer
 */

import { test } from "node:test";
import * as assert from "node:assert";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { FileOutputStreamer } from "./FileOutputStreamer";

test("FileOutputStreamer.stream watches log files and emits lines", async () => {
  const testDir = path.join(os.tmpdir(), `streamer-test-${Date.now()}`);
  fs.mkdirSync(testDir, { recursive: true });

  const streamer = new FileOutputStreamer();
  const taskId = "test-task";
  const lines: Array<{ line: string; source: "stdout" | "stderr" }> = [];

  // Start streaming (this creates empty log files)
  streamer.stream(taskId, testDir, (line, source) => {
    lines.push({ line, source });
  });

  // Wait a bit for watchers to be set up
  await new Promise((resolve) => setTimeout(resolve, 100));

  // Write to stdout.log
  const stdoutPath = path.join(testDir, "stdout.log");
  fs.writeFileSync(stdoutPath, "line 1\nline 2\n");

  // Wait for fs.watch to detect changes
  await new Promise((resolve) => setTimeout(resolve, 300));

  // Stop streaming
  streamer.stop(taskId);

  // Verify lines were captured
  assert.ok(lines.length >= 2, `Should capture at least 2 lines, got ${lines.length}`);
  if (lines.length >= 2) {
    assert.strictEqual(lines[0].line, "line 1");
    assert.strictEqual(lines[0].source, "stdout");
  }

  // Cleanup
  fs.rmSync(testDir, { recursive: true });
});

test("FileOutputStreamer.readAll returns all content", () => {
  const testDir = path.join(os.tmpdir(), `streamer-test-${Date.now()}`);
  fs.mkdirSync(testDir, { recursive: true });

  // Write test files
  fs.writeFileSync(path.join(testDir, "stdout.log"), "stdout content\n");
  fs.writeFileSync(path.join(testDir, "stderr.log"), "stderr content\n");

  const streamer = new FileOutputStreamer();
  const result = streamer.readAll(testDir);

  assert.strictEqual(result.stdout, "stdout content\n");
  assert.strictEqual(result.stderr, "stderr content\n");

  // Cleanup
  fs.rmSync(testDir, { recursive: true });
});

test("FileOutputStreamer.readFrom returns content from position", () => {
  const testDir = path.join(os.tmpdir(), `streamer-test-${Date.now()}`);
  fs.mkdirSync(testDir, { recursive: true });

  // Write test file
  const content = "line 1\nline 2\nline 3\n";
  fs.writeFileSync(path.join(testDir, "stdout.log"), content);

  const streamer = new FileOutputStreamer();
  const position = { byteOffset: 7, lineNumber: 1 }; // After "line 1\n"

  const result = streamer.readFrom(testDir, position);

  assert.strictEqual(result.stdout, "line 2\nline 3\n");
  assert.strictEqual(result.newPosition.byteOffset, content.length);

  // Cleanup
  fs.rmSync(testDir, { recursive: true });
});

test("FileOutputStreamer.stop removes watchers", async () => {
  const testDir = path.join(os.tmpdir(), `streamer-test-${Date.now()}`);
  fs.mkdirSync(testDir, { recursive: true });

  const streamer = new FileOutputStreamer();
  const taskId = "test-task";
  let callCount = 0;

  // Start streaming
  streamer.stream(taskId, testDir, () => {
    callCount++;
  });

  // Wait for watchers to be set up
  await new Promise((resolve) => setTimeout(resolve, 100));

  // Write initial content
  fs.writeFileSync(path.join(testDir, "stdout.log"), "line 1\n");
  await new Promise((resolve) => setTimeout(resolve, 300));

  const countAfterFirst = callCount;

  // Stop streaming
  streamer.stop(taskId);

  // Write more content (should not be captured)
  fs.appendFileSync(path.join(testDir, "stdout.log"), "line 2\n");
  await new Promise((resolve) => setTimeout(resolve, 300));

  // Verify no new lines were captured after stop
  assert.strictEqual(
    callCount,
    countAfterFirst,
    `Should not capture lines after stop (before: ${countAfterFirst}, after: ${callCount})`
  );

  // Cleanup
  fs.rmSync(testDir, { recursive: true });
});

test("FileOutputStreamer handles missing log files", () => {
  const testDir = path.join(os.tmpdir(), `streamer-test-${Date.now()}`);
  fs.mkdirSync(testDir, { recursive: true });

  const streamer = new FileOutputStreamer();
  const result = streamer.readAll(testDir);

  // Should return empty strings for missing files
  assert.strictEqual(result.stdout, "");
  assert.strictEqual(result.stderr, "");

  // Cleanup
  fs.rmSync(testDir, { recursive: true });
});
