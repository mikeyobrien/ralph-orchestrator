import Foundation

/// Response wrapper for the loops list endpoint.
struct LoopsResponse: Decodable {
    let loops: [LoopInfo]
}

/// Represents a running orchestration loop (primary or worktree).
/// Matches the JSON response from GET /api/loops and GET /api/loops/{id}
struct LoopInfo: Decodable, Identifiable {
    let id: String
    let status: String        // "primary" | "worktree"
    let prompt: String
    let pid: UInt32
    let startedAt: String
    let worktreePath: String?
    let workspace: String

    enum CodingKeys: String, CodingKey {
        case id, status, prompt, pid, workspace
        case startedAt = "started_at"
        case worktreePath = "worktree_path"
    }
}

/// Request body for spawning a new worktree loop.
struct SpawnLoopRequest: Encodable {
    let prompt: String
    let configPath: String?
    let baseBranch: String

    enum CodingKeys: String, CodingKey {
        case prompt
        case configPath = "config_path"
        case baseBranch = "base_branch"
    }
}

/// Response after successfully spawning a loop.
struct SpawnLoopResponse: Decodable {
    let id: String
    let worktreePath: String
    let status: String

    enum CodingKeys: String, CodingKey {
        case id, status
        case worktreePath = "worktree_path"
    }
}

/// Generic operation result for merge/discard actions.
struct OperationResponse: Decodable {
    let success: Bool
    let message: String
}
