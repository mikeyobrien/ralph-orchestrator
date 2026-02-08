import Foundation

/// Parses Server-Sent Events (SSE) stream format.
/// SSE format: `event: type\ndata: json\n\n`
struct SSEParser {
    private var eventType: String?
    private var dataBuffer: [String] = []

    /// Represents a parsed SSE message.
    struct SSEMessage {
        let eventType: String?
        let data: String
    }

    /// Parses a line from the SSE stream.
    /// Returns an SSEMessage when a complete event is received.
    ///
    /// Note: URLSession.AsyncBytes.lines omits empty lines, so we emit the event
    /// immediately after receiving the data line (since our server format is
    /// always `event: type\ndata: json\n\n`).
    mutating func parse(line: String) -> SSEMessage? {
        // Blank line signals end of event (for parsers that receive blank lines)
        if line.isEmpty {
            guard !dataBuffer.isEmpty else { return nil }

            let message = SSEMessage(
                eventType: eventType,
                data: dataBuffer.joined(separator: "\n")
            )

            // Reset state for next event
            eventType = nil
            dataBuffer.removeAll()

            return message
        }

        // Parse event type line
        if line.hasPrefix("event:") {
            let value = String(line.dropFirst(6)).trimmingCharacters(in: .whitespaces)
            eventType = value
            return nil
        }

        // Parse data line - emit event immediately since blank lines are omitted by URLSession
        if line.hasPrefix("data:") {
            let value = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)
            dataBuffer.append(value)

            // Emit the message immediately (URLSession.lines omits blank lines)
            let message = SSEMessage(
                eventType: eventType,
                data: dataBuffer.joined(separator: "\n")
            )

            // Reset state for next event
            eventType = nil
            dataBuffer.removeAll()

            return message
        }

        // Parse id line (not currently used but handled for completeness)
        if line.hasPrefix("id:") {
            return nil
        }

        // Parse retry line (not currently used but handled for completeness)
        if line.hasPrefix("retry:") {
            return nil
        }

        // Comments start with colon - ignore
        if line.hasPrefix(":") {
            return nil
        }

        return nil
    }

    /// Decodes an Event from SSE message data.
    static func decodeEvent(from data: String) -> Event? {
        guard let jsonData = data.data(using: .utf8) else {
            #if DEBUG
            print("[SSEParser] Failed to convert data to UTF8: \(data)")
            #endif
            return nil
        }

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601

        do {
            let event = try decoder.decode(Event.self, from: jsonData)
            #if DEBUG
            print("[SSEParser] Successfully decoded event: \(event.topic ?? "nil") type: \(event.type)")
            #endif
            return event
        } catch let decodingError as DecodingError {
            #if DEBUG
            switch decodingError {
            case .keyNotFound(let key, let context):
                print("[SSEParser] DECODE FAIL: Missing key '\(key.stringValue)' at \(context.codingPath)")
            case .typeMismatch(let type, let context):
                print("[SSEParser] DECODE FAIL: Type mismatch for \(type) at \(context.codingPath)")
            case .valueNotFound(let type, let context):
                print("[SSEParser] DECODE FAIL: Value not found for \(type) at \(context.codingPath)")
            case .dataCorrupted(let context):
                print("[SSEParser] DECODE FAIL: Data corrupted at \(context.codingPath): \(context.debugDescription)")
            @unknown default:
                print("[SSEParser] DECODE FAIL: Unknown error: \(decodingError)")
            }
            print("[SSEParser] Raw data: \(data)")
            #endif
            return nil
        } catch {
            #if DEBUG
            print("[SSEParser] DECODE FAIL: \(error)")
            print("[SSEParser] Raw data: \(data)")
            #endif
            return nil
        }
    }
}
