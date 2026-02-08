import Foundation

/// Response wrapper for the hats endpoint.
struct HatsResponse: Decodable {
    let hats: [HatItem]
}

/// Represents an orchestration role (hat) that Ralph can wear.
/// Matches the JSON response from GET /api/hats
struct HatItem: Decodable, Identifiable {
    let name: String
    let description: String
    let emoji: String

    var id: String { name }
}
