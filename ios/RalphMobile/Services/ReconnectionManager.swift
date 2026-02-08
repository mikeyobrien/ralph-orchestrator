import Foundation

/// Manages automatic reconnection with exponential backoff for SSE streams.
class ReconnectionManager {
    private var retryCount = 0
    private let maxRetries = 10
    private let baseDelay: TimeInterval = 1.0
    private let maxDelay: TimeInterval = 30.0

    /// Returns the next delay interval, or nil if max retries exceeded.
    /// Implements exponential backoff: 1s, 2s, 4s, 8s... capped at 30s.
    func nextDelay() -> TimeInterval? {
        guard retryCount < maxRetries else { return nil }
        let delay = baseDelay * pow(2.0, Double(retryCount))
        retryCount += 1
        return min(delay, maxDelay)
    }

    /// Resets retry count after successful connection.
    func reset() {
        retryCount = 0
    }

    /// Returns the current retry attempt number (1-based for display).
    var currentAttempt: Int {
        retryCount
    }
}
