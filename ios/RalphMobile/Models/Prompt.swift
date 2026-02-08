import Foundation

/// Represents a Ralph prompt file.
/// Matches the JSON response from GET /api/prompts
struct Prompt: Identifiable, Codable, Equatable, Hashable {
    /// Path to the prompt file relative to project root.
    let path: String
    /// Name derived from filename without extension.
    let name: String
    /// Preview extracted from first line, truncated to 50 characters.
    let preview: String

    /// Use path as the unique identifier for SwiftUI lists.
    var id: String { path }
}

/// Response wrapper for GET /api/prompts.
struct PromptsResponse: Codable {
    let prompts: [Prompt]
}
