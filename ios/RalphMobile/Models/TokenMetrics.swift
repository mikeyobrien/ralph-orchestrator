import Foundation

/// Aggregated token usage and cost metrics for a session.
struct TokenMetrics: Equatable {
    var inputTokens: Int = 0
    var outputTokens: Int = 0
    var estimatedCost: Double = 0.0
    var durationMs: Int?
    var maxTokens: Int = 100_000  // Default context window (100K)

    /// Total tokens (input + output).
    var totalTokens: Int {
        inputTokens + outputTokens
    }

    /// Reset all metrics to initial state.
    mutating func reset() {
        inputTokens = 0
        outputTokens = 0
        estimatedCost = 0.0
        durationMs = nil
    }

    /// Add token usage from an event payload.
    mutating func addUsage(input: Int, output: Int) {
        inputTokens += input
        outputTokens += output
    }
}

/// Parsed usage data from an Assistant event payload.
struct UsageData: Decodable {
    let inputTokens: Int
    let outputTokens: Int

    enum CodingKeys: String, CodingKey {
        case inputTokens = "input_tokens"
        case outputTokens = "output_tokens"
    }
}

/// Parsed result data from a Result event payload.
struct ResultData: Decodable {
    let totalCostUsd: Double?
    let durationMs: Int?

    enum CodingKeys: String, CodingKey {
        case totalCostUsd = "total_cost_usd"
        case durationMs = "duration_ms"
    }
}

/// Helper to parse event payloads for token metrics.
enum TokenMetricsParser {
    /// Extract usage data from an Assistant event payload.
    /// Expected format: {"type": "assistant", "usage": {"input_tokens": 100, "output_tokens": 50}}
    static func parseUsage(from payload: String?) -> UsageData? {
        guard let payload = payload,
              let data = payload.data(using: .utf8) else {
            return nil
        }

        struct AssistantPayload: Decodable {
            let usage: UsageData?
        }

        do {
            let decoded = try JSONDecoder().decode(AssistantPayload.self, from: data)
            return decoded.usage
        } catch {
            return nil
        }
    }

    /// Extract result data from a Result event payload.
    /// Expected format: {"total_cost_usd": 0.05, "duration_ms": 12345}
    static func parseResult(from payload: String?) -> ResultData? {
        guard let payload = payload,
              let data = payload.data(using: .utf8) else {
            return nil
        }

        do {
            return try JSONDecoder().decode(ResultData.self, from: data)
        } catch {
            return nil
        }
    }
}
