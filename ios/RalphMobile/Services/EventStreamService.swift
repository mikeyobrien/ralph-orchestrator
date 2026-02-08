import Foundation

/// Service for streaming events from the ralph-mobile-server via SSE.
actor EventStreamService {
    private let baseURL: URL
    private let session: URLSession

    init(baseURL: URL, apiKey: String) {
        self.baseURL = baseURL
        self.session = URLSession.shared
    }

    /// Connect to the SSE event stream for a specific session.
    /// Returns an AsyncStream of Events that yields as they arrive.
    func connect(sessionId: String) -> AsyncThrowingStream<Event, Error> {
        AsyncThrowingStream { continuation in
            Task {
                do {
                    try await self.streamEvents(sessionId: sessionId, continuation: continuation)
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    private func streamEvents(
        sessionId: String,
        continuation: AsyncThrowingStream<Event, Error>.Continuation
    ) async throws {
        let url = baseURL.appendingPathComponent("api/sessions/\(sessionId)/events")
        var request = URLRequest(url: url)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")

        let (bytes, response) = try await session.bytes(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw RalphError.networkError(URLError(.badServerResponse))
        }

        switch httpResponse.statusCode {
        case 200:
            break
        case 401:
            throw RalphError.unauthorized
        case 404:
            throw RalphError.sessionNotFound
        default:
            throw RalphError.serverError(statusCode: httpResponse.statusCode)
        }

        var parser = SSEParser()
        #if DEBUG
        print("[EventStreamService] Starting to read SSE lines...")
        #endif

        for try await line in bytes.lines {
            #if DEBUG
            print("[EventStreamService] Received line: \(line)")
            #endif
            if let message = parser.parse(line: line) {
                #if DEBUG
                print("[EventStreamService] Parsed message type: \(message.eventType ?? "none"), data: \(message.data)")
                #endif
                if let event = SSEParser.decodeEvent(from: message.data) {
                    #if DEBUG
                    print("[EventStreamService] Yielding event: \(event.topic)")
                    #endif
                    continuation.yield(event)
                } else {
                    #if DEBUG
                    print("[EventStreamService] Failed to decode event from data")
                    #endif
                }
            }
        }

        continuation.finish()
    }
}
