import Foundation

/// Response wrapper for the tasks list endpoint.
struct TasksResponse: Decodable {
    let tasks: [TaskItem]
    let total: Int
}

/// Represents a work item tracked by the orchestrator.
/// Matches the JSON response from GET /api/tasks
struct TaskItem: Decodable, Identifiable {
    let id: String
    let title: String
    let description: String?
    let status: String          // "open" | "in_progress" | "closed" | "failed"
    let priority: UInt8         // 1-5
    let blockedBy: [String]
    let loopId: String?
    let createdAt: String
    let updatedAt: String?

    enum CodingKeys: String, CodingKey {
        case id, title, description, status, priority
        case blockedBy = "blocked_by"
        case loopId = "loop_id"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

/// Request body for creating a new task.
struct CreateTaskRequest: Encodable {
    let title: String
    let description: String?
    let priority: UInt8
    let blockedBy: [String]

    enum CodingKeys: String, CodingKey {
        case title, description, priority
        case blockedBy = "blocked_by"
    }
}

/// Request body for updating a task's status.
struct UpdateTaskRequest: Encodable {
    let status: String
}
