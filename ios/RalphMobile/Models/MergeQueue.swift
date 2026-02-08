import Foundation

/// Response from the merge queue endpoint containing pending and completed items.
struct MergeQueueResponse: Decodable {
    let pending: [MergeQueueItem]
    let completed: [MergeQueueItem]
}

/// Represents a worktree loop queued for merge into the main branch.
/// Matches the JSON response from GET /api/merge-queue
struct MergeQueueItem: Decodable, Identifiable {
    let id: String
    let status: String
    let prompt: String
    let worktreePath: String?
    let queuedAt: String
    let mergedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, status, prompt
        case worktreePath = "worktree_path"
        case queuedAt = "queued_at"
        case mergedAt = "merged_at"
    }
}
