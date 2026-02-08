import Foundation

/// Represents a Ralph configuration preset.
/// Matches the JSON response from GET /api/configs
struct Config: Identifiable, Codable, Equatable, Hashable {
    /// Path to the config file relative to project root.
    let path: String
    /// Name derived from filename without extension.
    let name: String
    /// Description extracted from first comment line, empty if none.
    let description: String

    /// Use path as the unique identifier for SwiftUI lists.
    var id: String { path }
}

/// Response wrapper for GET /api/configs.
struct ConfigsResponse: Codable {
    let configs: [Config]
}
