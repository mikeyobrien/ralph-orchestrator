import Foundation

/// Errors that can occur during Anthropic API operations.
enum AnthropicError: Error, LocalizedError {
    case noAPIKey
    case invalidURL
    case invalidResponse
    case apiError(String)
    case networkError(Error)
    case decodingError(Error)

    var errorDescription: String? {
        switch self {
        case .noAPIKey:
            return "No Anthropic API key configured. Add it in Settings."
        case .invalidURL:
            return "Invalid API URL"
        case .invalidResponse:
            return "Invalid response from API"
        case .apiError(let message):
            return "API Error: \(message)"
        case .networkError(let error):
            return "Network error: \(error.localizedDescription)"
        case .decodingError(let error):
            return "Failed to decode response: \(error.localizedDescription)"
        }
    }
}

/// Actor for thread-safe Anthropic API interactions.
/// Provides AI-powered prompt improvement using Claude.
actor AnthropicClient {

    private let baseURL = "https://api.anthropic.com/v1/messages"
    private let model = "claude-3-5-sonnet-20241022"

    /// Shared singleton instance for app-wide access.
    static let shared = AnthropicClient()

    private init() {}

    /// Improve a user's prompt using Claude AI.
    /// - Parameters:
    ///   - prompt: The user's original prompt to improve.
    ///   - apiKey: The Anthropic API key.
    /// - Returns: An improved version of the prompt.
    /// - Throws: `AnthropicError` if the request fails.
    func improve(prompt: String, apiKey: String) async throws -> String {
        guard !apiKey.isEmpty else {
            throw AnthropicError.noAPIKey
        }

        guard let url = URL(string: baseURL) else {
            throw AnthropicError.invalidURL
        }

        let metaPrompt = buildMetaPrompt(userPrompt: prompt)

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(apiKey, forHTTPHeaderField: "x-api-key")
        request.setValue("2023-06-01", forHTTPHeaderField: "anthropic-version")

        let requestBody = AnthropicRequest(
            model: model,
            maxTokens: 2048,
            messages: [
                Message(role: "user", content: metaPrompt)
            ]
        )

        do {
            let encoder = JSONEncoder()
            encoder.keyEncodingStrategy = .convertToSnakeCase
            request.httpBody = try encoder.encode(requestBody)
        } catch {
            throw AnthropicError.decodingError(error)
        }

        let data: Data
        let response: URLResponse

        do {
            (data, response) = try await URLSession.shared.data(for: request)
        } catch {
            throw AnthropicError.networkError(error)
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            throw AnthropicError.invalidResponse
        }

        if httpResponse.statusCode != 200 {
            if let errorResponse = try? JSONDecoder().decode(AnthropicErrorResponse.self, from: data) {
                throw AnthropicError.apiError(errorResponse.error.message)
            }
            throw AnthropicError.apiError("HTTP \(httpResponse.statusCode)")
        }

        do {
            let decoder = JSONDecoder()
            decoder.keyDecodingStrategy = .convertFromSnakeCase
            let apiResponse = try decoder.decode(AnthropicResponse.self, from: data)

            guard let textContent = apiResponse.content.first(where: { $0.type == "text" }),
                  let text = textContent.text else {
                throw AnthropicError.invalidResponse
            }

            return text
        } catch let error as AnthropicError {
            throw error
        } catch {
            throw AnthropicError.decodingError(error)
        }
    }

    /// Build the meta-prompt that instructs Claude on how to improve the user's prompt.
    private func buildMetaPrompt(userPrompt: String) -> String {
        """
        You are a prompt engineering expert. Your task is to improve the following prompt to make it clearer, more specific, and more actionable.

        Guidelines for improvement:
        1. Add specific details where the original is vague
        2. Structure the request with clear sections if appropriate
        3. Include expected behavior and actual behavior for bug reports
        4. Add reproduction steps for debugging requests
        5. Clarify the scope and constraints
        6. Keep the user's original intent intact
        7. Don't add unnecessary complexity

        Original prompt:
        ---
        \(userPrompt)
        ---

        Provide ONLY the improved prompt text, without any explanation or commentary. The response should be ready to use directly as a prompt.
        """
    }
}

// MARK: - API Request/Response Models

private struct AnthropicRequest: Encodable {
    let model: String
    let maxTokens: Int
    let messages: [Message]
}

private struct Message: Encodable {
    let role: String
    let content: String
}

private struct AnthropicResponse: Decodable {
    let content: [ContentBlock]
}

private struct ContentBlock: Decodable {
    let type: String
    let text: String?
}

private struct AnthropicErrorResponse: Decodable {
    let error: APIError
}

private struct APIError: Decodable {
    let message: String
}
