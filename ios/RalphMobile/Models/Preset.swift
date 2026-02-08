import Foundation

/// Response wrapper for the presets endpoint.
struct PresetsResponse: Decodable {
    let presets: [PresetItem]
}

/// Represents a configuration preset file available for orchestration.
/// Matches the JSON response from GET /api/presets
struct PresetItem: Decodable, Identifiable {
    let name: String
    let path: String
    let description: String

    var id: String { path }
}
