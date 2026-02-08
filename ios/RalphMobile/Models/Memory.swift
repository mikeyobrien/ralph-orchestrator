import Foundation

/// Represents the memories file content and metadata.
/// Matches the JSON response from GET /api/memories and PUT /api/memories
struct MemoriesContent: Codable {
    let content: String
    let lastModified: String?

    enum CodingKeys: String, CodingKey {
        case content
        case lastModified = "last_modified"
    }
}

/// Request body for updating memories content.
struct UpdateMemoriesRequest: Encodable {
    let content: String
}

/// Response from exporting memories as a downloadable file.
struct MemoriesExport: Decodable {
    let content: String
    let exportedAt: String
    let filename: String

    enum CodingKeys: String, CodingKey {
        case content, filename
        case exportedAt = "exported_at"
    }
}
