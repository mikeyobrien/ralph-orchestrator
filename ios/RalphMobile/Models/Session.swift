import Foundation

/// Represents a Ralph orchestrator session being monitored.
/// Matches the JSON response from GET /api/sessions and GET /api/sessions/{id}/status
struct Session: Identifiable, Codable, Equatable, Hashable {
    let id: String
    var iteration: Int
    var total: Int?
    var hat: String?
    var elapsedSeconds: Int?  // Only present in status endpoint
    var mode: String?         // Only present in status endpoint
    var startedAt: String?    // Only present in list endpoint
    var status: String?       // "running", "paused", "stopped", "idle"
    var triggerEvent: String? // Event that triggered current hat
    var availablePublishes: [String]? // Events this hat can publish

    enum CodingKeys: String, CodingKey {
        case id
        case iteration
        case total
        case hat
        case elapsedSeconds = "elapsed_secs"
        case mode
        case startedAt = "started_at"
        case status
        case triggerEvent = "trigger_event"
        case availablePublishes = "publishes"
    }

    /// Computed property to get start time as Date
    var startTime: Date? {
        guard let startedAt = startedAt else { return nil }
        return Formatters.iso8601Formatter.date(from: startedAt)
    }
}

/// Backpressure check results
struct BackpressureStatus: Codable, Equatable {
    var testsPass: Bool
    var lintPass: Bool
    var typecheckPass: Bool

    enum CodingKeys: String, CodingKey {
        case testsPass = "tests"
        case lintPass = "lint"
        case typecheckPass = "typecheck"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        // Handle both bool and string values
        if let tests = try? container.decode(Bool.self, forKey: .testsPass) {
            testsPass = tests
        } else if let tests = try? container.decode(String.self, forKey: .testsPass) {
            testsPass = tests.lowercased() == "pass"
        } else {
            testsPass = false
        }

        if let lint = try? container.decode(Bool.self, forKey: .lintPass) {
            lintPass = lint
        } else if let lint = try? container.decode(String.self, forKey: .lintPass) {
            lintPass = lint.lowercased() == "pass"
        } else {
            lintPass = false
        }

        if let typecheck = try? container.decode(Bool.self, forKey: .typecheckPass) {
            typecheckPass = typecheck
        } else if let typecheck = try? container.decode(String.self, forKey: .typecheckPass) {
            typecheckPass = typecheck.lowercased() == "pass"
        } else {
            typecheckPass = false
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(testsPass, forKey: .testsPass)
        try container.encode(lintPass, forKey: .lintPass)
        try container.encode(typecheckPass, forKey: .typecheckPass)
    }

    init(testsPass: Bool = false, lintPass: Bool = false, typecheckPass: Bool = false) {
        self.testsPass = testsPass
        self.lintPass = lintPass
        self.typecheckPass = typecheckPass
    }
}
