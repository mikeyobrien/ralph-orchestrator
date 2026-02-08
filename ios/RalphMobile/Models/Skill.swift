import Foundation

/// Represents a skill from the Ralph orchestrator.
/// Matches the JSON response from GET /api/skills
struct Skill: Identifiable, Codable, Equatable, Hashable {
    let name: String
    let description: String
    let tags: [String]
    let hats: [String]
    let backends: [String]
    let autoInject: Bool
    let source: String  // "built-in" | "file"

    var id: String { name }

    enum CodingKeys: String, CodingKey {
        case name
        case description
        case tags
        case hats
        case backends
        case autoInject = "auto_inject"
        case source
    }

    /// Whether this is a built-in skill
    var isBuiltIn: Bool {
        source == "built-in"
    }

    /// Icon for the skill source type
    var sourceIcon: String {
        isBuiltIn ? "cube.fill" : "doc.text"
    }
}

/// Response for GET /api/skills
struct SkillsListResponse: Decodable {
    let skills: [Skill]
    let count: Int
}

/// Response for GET /api/skills/{name}
struct SkillMetadataResponse: Decodable {
    let name: String
    let description: String
    let tags: [String]
    let hats: [String]
    let backends: [String]
    let autoInject: Bool
    let source: String

    enum CodingKeys: String, CodingKey {
        case name
        case description
        case tags
        case hats
        case backends
        case autoInject = "auto_inject"
        case source
    }
}

/// Response for POST /api/skills/{name}/load
struct SkillContentResponse: Decodable {
    let name: String
    let content: String  // XML-wrapped content
}
