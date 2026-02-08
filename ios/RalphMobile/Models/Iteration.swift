import Foundation

/// Response wrapper for the iterations endpoint.
struct IterationsResponse: Decodable {
    let iterations: [IterationItem]
    let total: Int
}

/// Represents a single iteration within a session's execution history.
/// Matches the JSON response from GET /api/sessions/{id}/iterations
struct IterationItem: Decodable, Identifiable {
    let number: UInt32
    let hat: String?
    let startedAt: String
    let durationSecs: UInt64?

    var id: UInt32 { number }

    enum CodingKeys: String, CodingKey {
        case number, hat
        case startedAt = "started_at"
        case durationSecs = "duration_secs"
    }
}
