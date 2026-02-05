import { describe, it, beforeEach } from "node:test";
import assert from "node:assert";
import { HatsEventParser, HatsEvent } from "./HatsEventParser";

describe("HatsEventParser", () => {
  let parser: HatsEventParser;
  let parsedEvents: HatsEvent[];

  beforeEach(() => {
    parsedEvents = [];
    parser = new HatsEventParser((event) => {
      parsedEvents.push(event);
    });
  });

  describe("parseLine", () => {
    it("should parse a valid event JSON line", () => {
      const line = '{"ts":"2026-01-26T10:00:00.000Z","topic":"build.done","payload":"tests pass"}';

      parser.parseLine(line);

      assert.strictEqual(parsedEvents.length, 1);
      assert.strictEqual(parsedEvents[0].topic, "build.done");
      assert.strictEqual(parsedEvents[0].payload, "tests pass");
      assert.strictEqual(parsedEvents[0].ts, "2026-01-26T10:00:00.000Z");
    });

    it("should parse event with JSON payload", () => {
      const line =
        '{"ts":"2026-01-26T10:00:00.000Z","topic":"confession.clean","payload":{"confidence":95,"summary":"all good"}}';

      parser.parseLine(line);

      assert.strictEqual(parsedEvents.length, 1);
      assert.strictEqual(parsedEvents[0].topic, "confession.clean");
      assert.deepStrictEqual(parsedEvents[0].payload, { confidence: 95, summary: "all good" });
    });

    it("should ignore non-JSON lines", () => {
      parser.parseLine("This is just a regular log line");
      parser.parseLine("Another log line with no JSON");

      assert.strictEqual(parsedEvents.length, 0);
    });

    it("should ignore JSON without topic field", () => {
      parser.parseLine('{"foo":"bar","baz":123}');

      assert.strictEqual(parsedEvents.length, 0);
    });

    it("should ignore JSON with non-string topic", () => {
      parser.parseLine('{"topic":123,"payload":"test"}');

      assert.strictEqual(parsedEvents.length, 0);
    });

    it("should parse event with optional fields", () => {
      const line =
        '{"ts":"2026-01-26T10:00:00.000Z","iteration":5,"hat":"builder","topic":"build.done","triggered":"plan.start","payload":"done"}';

      parser.parseLine(line);

      assert.strictEqual(parsedEvents.length, 1);
      assert.strictEqual(parsedEvents[0].iteration, 5);
      assert.strictEqual(parsedEvents[0].hat, "builder");
      assert.strictEqual(parsedEvents[0].triggered, "plan.start");
    });

    it("should handle event with empty payload", () => {
      const line = '{"ts":"2026-01-26T10:00:00.000Z","topic":"plan.start","payload":""}';

      parser.parseLine(line);

      assert.strictEqual(parsedEvents.length, 1);
      assert.strictEqual(parsedEvents[0].payload, "");
    });

    it("should handle event with null payload", () => {
      const line = '{"ts":"2026-01-26T10:00:00.000Z","topic":"plan.start","payload":null}';

      parser.parseLine(line);

      assert.strictEqual(parsedEvents.length, 1);
      assert.strictEqual(parsedEvents[0].payload, null);
    });

    it("should ignore malformed JSON", () => {
      parser.parseLine('{"topic":"build.done"'); // Missing closing brace
      parser.parseLine('{topic: "build.done"}'); // Invalid JSON (unquoted key)

      assert.strictEqual(parsedEvents.length, 0);
    });

    it("should handle multiple events in sequence", () => {
      parser.parseLine('{"ts":"2026-01-26T10:00:00.000Z","topic":"build.task","payload":"task1"}');
      parser.parseLine("Regular log line in between");
      parser.parseLine(
        '{"ts":"2026-01-26T10:00:01.000Z","topic":"build.done","payload":"completed"}'
      );

      assert.strictEqual(parsedEvents.length, 2);
      assert.strictEqual(parsedEvents[0].topic, "build.task");
      assert.strictEqual(parsedEvents[1].topic, "build.done");
    });
  });

  describe("isEventLine", () => {
    it("should return true for event lines", () => {
      assert.strictEqual(
        HatsEventParser.isEventLine(
          '{"ts":"2026-01-26T10:00:00.000Z","topic":"build.done","payload":""}'
        ),
        true
      );
    });

    it("should return false for regular log lines", () => {
      assert.strictEqual(HatsEventParser.isEventLine("Just a log message"), false);
    });

    it("should return false for JSON without topic", () => {
      assert.strictEqual(HatsEventParser.isEventLine('{"foo":"bar"}'), false);
    });
  });
});
