import Foundation

/// Represents a workflow event from the Ralph event stream.
/// Supports both workflow events (topic/payload) and tool calls (tool/input/output).
struct Event: Identifiable, Codable, Equatable {
    // MARK: - Common Properties
    let timestamp: Date

    // MARK: - Workflow Event Properties
    let topic: String?
    let payload: String?
    let iteration: Int?
    let hat: String?
    let triggered: String?

    // MARK: - Tool Call Properties (from claude code streaming)
    let type: String // "tool.call", "hat.activated", "event.published", "backpressure"
    let toolName: String?
    let status: String? // "running", "completed", "pending", "error"
    let input: [String: Any]?
    let output: String?
    let duration: Int? // milliseconds

    // MARK: - Computed Properties
    var id: String {
        if let tool = toolName {
            return "\(tool)-\(timestamp.timeIntervalSince1970)"
        }
        return "\(topic ?? type)-\(timestamp.timeIntervalSince1970)"
    }

    /// Categorizes the event for display purposes
    var eventCategory: EventCategory {
        // Tool calls
        if toolName != nil || type == "tool.call" {
            return .tool
        }

        // Hat transitions
        if type == "hat.activated" || (topic?.contains("hat") ?? false) {
            return .hat
        }

        guard let topic = topic?.lowercased() else {
            return .debug
        }

        // Backpressure / Gates
        if topic.contains("blocked") || topic.contains("failed") ||
           topic.contains("error") || topic.contains("passed") ||
           type == "backpressure" {
            return .gate
        }

        // Task lifecycle
        if topic.contains("task.") || topic.contains("loop.") ||
           topic.contains("iteration") || topic.contains("start") ||
           topic.contains("complete") || topic.contains("terminate") {
            return .task
        }

        return .debug
    }

    /// Display priority for visual hierarchy
    var displayPriority: EventPriority {
        switch eventCategory {
        case .hat, .gate, .task:
            return .high
        case .tool:
            return .medium
        case .debug:
            return .low
        }
    }

    /// Human-readable title for the event
    var humanReadableTitle: String {
        // Tool calls
        if let tool = toolName {
            return "Tool: \(tool)"
        }

        guard let topic = topic else {
            return type.replacingOccurrences(of: ".", with: " ").capitalized
        }

        // Parse common topic patterns
        let parts = topic.split(separator: ".")
        if parts.count >= 2 {
            let category = String(parts[0]).capitalized
            let action = String(parts[1]).capitalized
            return "\(category) \(action)"
        }

        return topic.replacingOccurrences(of: ".", with: " ").capitalized
    }

    /// Smart summary for long payloads (truncates intelligently)
    var smartSummary: String? {
        guard let payload = payload, !payload.isEmpty else { return nil }

        // Short payloads don't need summarization
        if payload.count <= 200 {
            return payload
        }

        // Extract key information based on topic
        if let topic = topic?.lowercased() {
            // Task start - extract objective
            if topic.contains("task.start") || topic.contains("task.started") {
                return extractTaskObjective(from: payload)
            }

            // Loop termination - extract summary
            if topic.contains("loop.terminate") || topic.contains("loop.complete") {
                return extractCompletionSummary(from: payload)
            }

            // Errors - extract error message
            if topic.contains("error") || topic.contains("blocked") || topic.contains("failed") {
                return extractErrorMessage(from: payload)
            }
        }

        // Default: first 200 chars with ellipsis
        return String(payload.prefix(200)) + "..."
    }

    /// Whether this event represents an error/failure
    var isError: Bool {
        if status == "error" { return true }
        guard let topic = topic?.lowercased() else { return false }
        return topic.contains("error") || topic.contains("blocked") || topic.contains("failed")
    }

    /// Whether this event represents success
    var isSuccess: Bool {
        if status == "completed" { return true }
        guard let topic = topic?.lowercased() else { return false }
        return topic.contains("passed") || topic.contains("complete") || topic.contains("success")
    }

    // MARK: - Private Helpers

    private func extractTaskObjective(from payload: String) -> String {
        // Look for ## Objective section
        if let range = payload.range(of: "## Objective") {
            let afterObjective = payload[range.upperBound...]
            // Find next ## or take first 200 chars
            if let nextSection = afterObjective.range(of: "\n##") {
                let objective = String(afterObjective[..<nextSection.lowerBound])
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                if objective.count > 200 {
                    return String(objective.prefix(200)) + "..."
                }
                return objective
            }
            return String(afterObjective.prefix(200)).trimmingCharacters(in: .whitespacesAndNewlines) + "..."
        }

        // Look for first paragraph after # heading
        if let firstPara = payload.components(separatedBy: "\n\n").dropFirst().first {
            if firstPara.count > 200 {
                return String(firstPara.prefix(200)) + "..."
            }
            return firstPara
        }

        return String(payload.prefix(200)) + "..."
    }

    private func extractCompletionSummary(from payload: String) -> String {
        // Look for ## Summary section
        if let range = payload.range(of: "## Summary") {
            let afterSummary = payload[range.upperBound...]
            if let nextSection = afterSummary.range(of: "\n##") {
                return String(afterSummary[..<nextSection.lowerBound])
                    .trimmingCharacters(in: .whitespacesAndNewlines)
            }
            return String(afterSummary.prefix(200)).trimmingCharacters(in: .whitespacesAndNewlines)
        }

        // Look for reason/status
        var summary = ""
        if payload.contains("Reason") {
            if let line = payload.components(separatedBy: "\n").first(where: { $0.contains("Reason") }) {
                summary += line.trimmingCharacters(in: .whitespacesAndNewlines)
            }
        }

        return summary.isEmpty ? String(payload.prefix(200)) + "..." : summary
    }

    private func extractErrorMessage(from payload: String) -> String {
        // Try to find error-specific content
        let lines = payload.components(separatedBy: "\n")

        // Look for lines with "error", "Error", "failed", etc.
        for line in lines {
            let lower = line.lowercased()
            if lower.contains("error:") || lower.contains("failed:") ||
               lower.contains("cannot") || lower.contains("missing") {
                let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
                if trimmed.count > 200 {
                    return String(trimmed.prefix(200)) + "..."
                }
                return trimmed
            }
        }

        return String(payload.prefix(200)) + "..."
    }

    // MARK: - Codable

    enum CodingKeys: String, CodingKey {
        case timestamp = "ts"
        case topic
        case payload
        case iteration
        case hat
        case triggered
        case type
        case toolName = "tool"
        case status
        case input
        case output
        case duration
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)

        // Timestamp - try multiple formats
        if let tsString = try? container.decode(String.self, forKey: .timestamp) {
            // Try with fractional seconds first
            if let date = Formatters.iso8601Formatter.date(from: tsString) {
                timestamp = date
            } else if let date = Formatters.iso8601FormatterNoFractional.date(from: tsString) {
                // Try without fractional seconds
                timestamp = date
            } else {
                // Last resort: create a flexible formatter for edge cases
                let flexFormatter = ISO8601DateFormatter()
                flexFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds, .withTimeZone]
                if let date = flexFormatter.date(from: tsString) {
                    timestamp = date
                } else {
                    #if DEBUG
                    print("[Event] WARNING: Could not parse timestamp: \(tsString), using Date()")
                    #endif
                    timestamp = Date()
                }
            }
        } else if let ts = try? container.decode(Date.self, forKey: .timestamp) {
            timestamp = ts
        } else {
            #if DEBUG
            print("[Event] WARNING: No timestamp field, using Date()")
            #endif
            timestamp = Date()
        }

        topic = try? container.decode(String.self, forKey: .topic)
        #if DEBUG
        if topic == nil {
            print("[Event] WARNING: No topic field in event")
        }
        #endif
        payload = try? container.decode(String.self, forKey: .payload)
        iteration = try? container.decode(Int.self, forKey: .iteration)
        hat = try? container.decode(String.self, forKey: .hat)
        triggered = try? container.decode(String.self, forKey: .triggered)

        // Type defaults to "event" for workflow events
        type = (try? container.decode(String.self, forKey: .type)) ?? "event.published"
        toolName = try? container.decode(String.self, forKey: .toolName)
        status = try? container.decode(String.self, forKey: .status)
        duration = try? container.decode(Int.self, forKey: .duration)
        output = try? container.decode(String.self, forKey: .output)

        // Input can be a dictionary - decode as JSON
        if let inputData = try? container.decode([String: AnyCodable].self, forKey: .input) {
            input = inputData.mapValues { $0.value }
        } else {
            input = nil
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(timestamp, forKey: .timestamp)
        try container.encodeIfPresent(topic, forKey: .topic)
        try container.encodeIfPresent(payload, forKey: .payload)
        try container.encodeIfPresent(iteration, forKey: .iteration)
        try container.encodeIfPresent(hat, forKey: .hat)
        try container.encodeIfPresent(triggered, forKey: .triggered)
        try container.encode(type, forKey: .type)
        try container.encodeIfPresent(toolName, forKey: .toolName)
        try container.encodeIfPresent(status, forKey: .status)
        try container.encodeIfPresent(duration, forKey: .duration)
        try container.encodeIfPresent(output, forKey: .output)

        if let input = input {
            let codableInput = input.mapValues { AnyCodable($0) }
            try container.encode(codableInput, forKey: .input)
        }
    }

    // MARK: - Equatable

    static func == (lhs: Event, rhs: Event) -> Bool {
        lhs.id == rhs.id
    }

    // MARK: - Convenience Initializer

    init(
        timestamp: Date = Date(),
        topic: String? = nil,
        payload: String? = nil,
        iteration: Int? = nil,
        hat: String? = nil,
        triggered: String? = nil,
        type: String = "event.published",
        toolName: String? = nil,
        status: String? = nil,
        input: [String: Any]? = nil,
        output: String? = nil,
        duration: Int? = nil
    ) {
        self.timestamp = timestamp
        self.topic = topic
        self.payload = payload
        self.iteration = iteration
        self.hat = hat
        self.triggered = triggered
        self.type = type
        self.toolName = toolName
        self.status = status
        self.input = input
        self.output = output
        self.duration = duration
    }
}

// MARK: - AnyCodable Helper

/// Type-erased Codable wrapper for dictionary values
struct AnyCodable: Codable {
    let value: Any

    init(_ value: Any) {
        self.value = value
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if container.decodeNil() {
            value = NSNull()
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else if let string = try? container.decode(String.self) {
            value = string
        } else if let array = try? container.decode([AnyCodable].self) {
            value = array.map { $0.value }
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            value = dict.mapValues { $0.value }
        } else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Cannot decode AnyCodable")
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()

        switch value {
        case is NSNull:
            try container.encodeNil()
        case let bool as Bool:
            try container.encode(bool)
        case let int as Int:
            try container.encode(int)
        case let double as Double:
            try container.encode(double)
        case let string as String:
            try container.encode(string)
        case let array as [Any]:
            try container.encode(array.map { AnyCodable($0) })
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodable($0) })
        default:
            try container.encode(String(describing: value))
        }
    }
}

// MARK: - Event Category

/// Categories for grouping events by type
enum EventCategory: String, CaseIterable {
    case hat = "hat"         // Hat transitions
    case gate = "gate"       // Backpressure / gates (build, test, lint)
    case task = "task"       // Task lifecycle (start, complete, terminate)
    case tool = "tool"       // Tool calls (Read, Edit, Glob, etc.)
    case debug = "debug"     // Debug/progress messages

    var icon: String {
        switch self {
        case .hat: return "person.crop.circle.badge.checkmark"
        case .gate: return "door.left.hand.closed"
        case .task: return "target"
        case .tool: return "wrench.and.screwdriver"
        case .debug: return "text.alignleft"
        }
    }

    var label: String {
        switch self {
        case .hat: return "HAT"
        case .gate: return "GATE"
        case .task: return "TASK"
        case .tool: return "TOOL"
        case .debug: return "DEBUG"
        }
    }
}

// MARK: - Event Priority

/// Display priority for visual hierarchy
enum EventPriority: Int, Comparable {
    case high = 3    // Full card display
    case medium = 2  // Compact row
    case low = 1     // Minimal/grouped

    static func < (lhs: EventPriority, rhs: EventPriority) -> Bool {
        lhs.rawValue < rhs.rawValue
    }
}
