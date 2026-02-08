import Foundation

/// Response wrapper for the RObot questions endpoint.
struct QuestionsResponse: Decodable {
    let questions: [PendingQuestion]
}

/// Represents a pending human-in-the-loop question from an agent.
/// Matches the JSON response from GET /api/robot/questions
struct PendingQuestion: Decodable, Identifiable {
    let id: String
    let questionText: String
    let sessionId: String
    let askedAt: String
    let timeoutAt: String
    let iteration: UInt32
    let hat: String?

    enum CodingKeys: String, CodingKey {
        case id, iteration, hat
        case questionText = "question_text"
        case sessionId = "session_id"
        case askedAt = "asked_at"
        case timeoutAt = "timeout_at"
    }
}

/// Request body for responding to a pending question.
struct QuestionResponseRequest: Encodable {
    let questionId: String
    let responseText: String

    enum CodingKeys: String, CodingKey {
        case questionId = "question_id"
        case responseText = "response_text"
    }
}

/// Acknowledgement after submitting a question response.
struct ResponseAck: Decodable {
    let success: Bool
    let questionId: String
    let deliveredAt: String

    enum CodingKeys: String, CodingKey {
        case success
        case questionId = "question_id"
        case deliveredAt = "delivered_at"
    }
}

/// Request body for sending proactive guidance to a session.
struct GuidanceRequest: Encodable {
    let sessionId: String
    let guidanceText: String

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case guidanceText = "guidance_text"
    }
}

/// Acknowledgement after sending guidance.
struct GuidanceAck: Decodable {
    let success: Bool
    let sessionId: String
    let deliveredAt: String

    enum CodingKeys: String, CodingKey {
        case success
        case sessionId = "session_id"
        case deliveredAt = "delivered_at"
    }
}
