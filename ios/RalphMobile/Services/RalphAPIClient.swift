import Foundation

/// Request body for starting a new session.
struct StartSessionRequest: Encodable {
    let configPath: String
    let promptPath: String
    let workingDir: String?

    enum CodingKeys: String, CodingKey {
        case configPath = "config_path"
        case promptPath = "prompt_path"
        case workingDir = "working_dir"
    }
}

/// Response body after starting a session.
struct StartSessionResponse: Decodable {
    let id: String
    let status: String
}

/// Error types for Ralph API operations.
enum RalphError: Error, LocalizedError {
    case invalidURL
    case unauthorized
    case sessionNotFound
    case configNotFound(String)
    case promptNotFound(String)
    case spawnFailed(String)
    case invalidPath(String)
    case invalidTopic(String)
    case serverError(statusCode: Int)
    case networkError(Error)
    case decodingError(Error)

    var errorDescription: String? {
        switch self {
        case .invalidURL:
            return "Invalid server URL"
        case .unauthorized:
            return "Invalid API key"
        case .sessionNotFound:
            return "Session not found"
        case .configNotFound(let path):
            return "Config not found: \(path)"
        case .promptNotFound(let path):
            return "Prompt not found: \(path)"
        case .spawnFailed(let reason):
            return "Failed to start session: \(reason)"
        case .invalidPath(let path):
            return "Invalid path: \(path)"
        case .invalidTopic(let topic):
            return "Invalid topic: \(topic)"
        case .serverError(let code):
            return "Server error: \(code)"
        case .networkError(let error):
            return "Network error: \(error.localizedDescription)"
        case .decodingError(let error):
            return "Failed to decode response: \(error.localizedDescription)"
        }
    }
}

/// Response body for config content retrieval.
struct ConfigContentResponse: Decodable {
    let path: String
    let content: String
    let contentType: String

    enum CodingKeys: String, CodingKey {
        case path
        case content
        case contentType = "content_type"
    }
}

/// Response body for prompt content retrieval.
struct PromptContentResponse: Decodable {
    let path: String
    let content: String
    let contentType: String

    enum CodingKeys: String, CodingKey {
        case path
        case content
        case contentType = "content_type"
    }
}

/// Request body for emitting events.
struct EmitEventRequest: Encodable {
    let topic: String
    let payload: String?
}

/// Response body after emitting an event.
struct EmitEventResponse: Decodable {
    let success: Bool
    let topic: String
    let timestamp: String
}

/// Request body for steering a session.
struct SteerRequest: Encodable {
    let message: String
}

/// Response body for scratchpad content.
struct ScratchpadResponse: Decodable {
    let content: String
}

// MARK: - Host Metrics Models

/// CPU metrics from the host.
struct CpuMetrics: Decodable {
    let usagePercent: Double
    let cores: Int

    enum CodingKeys: String, CodingKey {
        case usagePercent = "usage_percent"
        case cores
    }
}

/// Memory metrics from the host.
struct MemoryMetrics: Decodable {
    let usagePercent: Double
    let usedGb: Double
    let totalGb: Double

    enum CodingKeys: String, CodingKey {
        case usagePercent = "usage_percent"
        case usedGb = "used_gb"
        case totalGb = "total_gb"
    }
}

/// Disk metrics from the host.
struct DiskMetrics: Decodable {
    let usagePercent: Double
    let usedGb: Double
    let totalGb: Double

    enum CodingKeys: String, CodingKey {
        case usagePercent = "usage_percent"
        case usedGb = "used_gb"
        case totalGb = "total_gb"
    }
}

/// Network metrics from the host.
struct NetworkMetrics: Decodable {
    let downloadMbps: Double
    let uploadMbps: Double

    enum CodingKeys: String, CodingKey {
        case downloadMbps = "download_mbps"
        case uploadMbps = "upload_mbps"
    }
}

/// Host metrics response from the API.
struct HostMetricsResponse: Decodable {
    let cpu: CpuMetrics
    let memory: MemoryMetrics
    let disk: DiskMetrics
    let network: NetworkMetrics
}

/// Process information from the host.
struct ProcessInfo: Decodable, Identifiable {
    let name: String
    let cpuPercent: Double
    let memoryMb: Double
    let pid: UInt32

    var id: UInt32 { pid }

    enum CodingKeys: String, CodingKey {
        case name
        case cpuPercent = "cpu_percent"
        case memoryMb = "memory_mb"
        case pid
    }
}

/// Processes response from the API.
struct ProcessesResponse: Decodable {
    let processes: [ProcessInfo]
}

/// REST API client for communicating with ralph-mobile-server.
actor RalphAPIClient {
    // MARK: - Shared Singleton

    /// Shared singleton instance for V5 unified architecture.
    /// Configure with `RalphAPIClient.configure(baseURL:apiKey:)` before use.
    private static var _shared: RalphAPIClient?
    private static let lock = NSLock()

    /// Access the shared instance. Must call `configure` first.
    static var shared: RalphAPIClient {
        lock.lock()
        defer { lock.unlock() }
        guard let instance = _shared else {
            fatalError("RalphAPIClient.shared accessed before configure() was called")
        }
        return instance
    }

    /// Configure the shared singleton with server credentials.
    static func configure(baseURL: URL, apiKey: String) {
        lock.lock()
        defer { lock.unlock() }
        _shared = RalphAPIClient(baseURL: baseURL, apiKey: apiKey)
    }

    /// Check health using the shared instance.
    static func checkHealth() async -> Bool {
        await shared.checkHealth()
    }

    /// Check if the shared instance has been configured.
    static var isConfigured: Bool {
        lock.lock()
        defer { lock.unlock() }
        return _shared != nil
    }

    // MARK: - Instance Properties

    private let baseURL: URL
    private let apiKey: String
    private let session: URLSession
    private let decoder: JSONDecoder

    init(baseURL: URL, apiKey: String) {
        self.baseURL = baseURL
        self.apiKey = apiKey
        self.session = URLSession.shared

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        self.decoder = decoder
    }

    /// Check if the server is reachable by calling GET /health.
    /// Returns true if server responds with 200, false otherwise.
    /// Uses a 3-second timeout to fail fast.
    func checkHealth() async -> Bool {
        guard let url = URL(string: "\(baseURL.absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/")))/health") else {
            return false
        }
        var request = URLRequest(url: url)
        request.timeoutInterval = 3
        do {
            let (_, response) = try await URLSession.shared.data(for: request)
            return (response as? HTTPURLResponse)?.statusCode == 200
        } catch {
            return false
        }
    }

    /// Fetch all active sessions.
    func getSessions() async throws -> [Session] {
        let url = baseURL.appendingPathComponent("api/sessions")
        return try await performRequest(url: url)
    }

    /// Fetch status for a specific session.
    func getSessionStatus(id: String) async throws -> Session {
        let url = baseURL.appendingPathComponent("api/sessions/\(id)/status")
        return try await performRequest(url: url)
    }

    /// Fetch all available configuration presets.
    func getConfigs() async throws -> [Config] {
        let url = baseURL.appendingPathComponent("api/configs")
        let response: ConfigsResponse = try await performRequest(url: url)
        return response.configs
    }

    /// Fetch all available prompt files.
    func getPrompts() async throws -> [Prompt] {
        let url = baseURL.appendingPathComponent("api/prompts")
        let response: PromptsResponse = try await performRequest(url: url)
        return response.prompts
    }

    /// Start a new Ralph session with the given config and prompt.
    func startSession(config: String, prompt: String, workingDir: String?) async throws -> StartSessionResponse {
        let url = baseURL.appendingPathComponent("api/sessions")
        let body = StartSessionRequest(configPath: config, promptPath: prompt, workingDir: workingDir)
        return try await performPostRequest(url: url, body: body)
    }

    /// Create a new Ralph session and return the full Session object.
    /// This is a convenience method that starts a session and fetches its full details.
    func createSession(config: String, prompt: String, directory: String) async throws -> Session {
        let response = try await startSession(config: config, prompt: prompt, workingDir: directory.isEmpty ? nil : directory)
        return try await getSessionStatus(id: response.id)
    }

    /// Stop a running Ralph session.
    func stopSession(id: String) async throws {
        let url = baseURL.appendingPathComponent("api/sessions/\(id)")
        try await performDeleteRequest(url: url)
    }

    /// Fetch raw content of a config file.
    func getConfigContent(path: String) async throws -> ConfigContentResponse {
        let url = baseURL.appendingPathComponent("api/configs/\(path)")
        return try await performRequest(url: url)
    }

    /// Fetch raw content of a prompt file.
    func getPromptContent(path: String) async throws -> PromptContentResponse {
        let url = baseURL.appendingPathComponent("api/prompts/\(path)")
        return try await performRequest(url: url)
    }

    /// Emit an event to a running session.
    func emitEvent(sessionId: String, topic: String, payload: String? = nil) async throws -> EmitEventResponse {
        let url = baseURL.appendingPathComponent("api/sessions/\(sessionId)/emit")
        let body = EmitEventRequest(topic: topic, payload: payload)
        return try await performPostRequest(url: url, body: body)
    }

    /// Pause a running session.
    func pauseSession(id: String) async throws {
        let url = baseURL.appendingPathComponent("api/sessions/\(id)/pause")
        let _: EmptyResponse = try await performPostRequestNoBody(url: url)
    }

    /// Resume a paused session.
    func resumeSession(id: String) async throws {
        let url = baseURL.appendingPathComponent("api/sessions/\(id)/resume")
        let _: EmptyResponse = try await performPostRequestNoBody(url: url)
    }

    /// Send a steering message to a running session.
    func steerSession(id: String, message: String) async throws {
        let url = baseURL.appendingPathComponent("api/sessions/\(id)/steer")
        let body = SteerRequest(message: message)
        let _: EmptyResponse = try await performPostRequest(url: url, body: body)
    }

    /// Fetch scratchpad content for a session.
    func getScratchpad(sessionId: String) async throws -> String {
        let url = baseURL.appendingPathComponent("api/sessions/\(sessionId)/scratchpad")
        let response: ScratchpadResponse = try await performRequest(url: url)
        return response.content
    }

    /// Fetch host metrics (CPU, memory, disk, network).
    func getHostMetrics() async throws -> HostMetricsResponse {
        let url = baseURL.appendingPathComponent("api/host/metrics")
        return try await performRequest(url: url)
    }

    /// Fetch top processes by CPU usage.
    func getHostProcesses() async throws -> [ProcessInfo] {
        let url = baseURL.appendingPathComponent("api/host/processes")
        let response: ProcessesResponse = try await performRequest(url: url)
        return response.processes
    }

    // MARK: - Skills API

    /// Fetch all available skills.
    func getSkills() async throws -> [Skill] {
        let url = baseURL.appendingPathComponent("api/skills")
        let response: SkillsListResponse = try await performRequest(url: url)
        return response.skills
    }

    /// Fetch metadata for a specific skill.
    func getSkill(name: String) async throws -> Skill {
        let url = baseURL.appendingPathComponent("api/skills/\(name)")
        return try await performRequest(url: url)
    }

    /// Load full skill content (XML-wrapped).
    func loadSkill(name: String) async throws -> SkillContentResponse {
        let url = baseURL.appendingPathComponent("api/skills/\(name)/load")
        return try await performPostRequestNoBody(url: url)
    }

    // MARK: - Loops API

    /// Fetch all running orchestration loops.
    func getLoops() async throws -> [LoopInfo] {
        let url = baseURL.appendingPathComponent("api/loops")
        let response: LoopsResponse = try await performRequest(url: url)
        return response.loops
    }

    /// Fetch details for a specific loop.
    func getLoop(id: String) async throws -> LoopInfo {
        let url = baseURL.appendingPathComponent("api/loops/\(id)")
        return try await performRequest(url: url)
    }

    /// Spawn a new worktree loop.
    func spawnLoop(prompt: String, configPath: String?, baseBranch: String = "main") async throws -> SpawnLoopResponse {
        let url = baseURL.appendingPathComponent("api/loops")
        let body = SpawnLoopRequest(prompt: prompt, configPath: configPath, baseBranch: baseBranch)
        return try await performPostRequest(url: url, body: body)
    }

    /// Queue a worktree loop for merge into main.
    func mergeLoop(id: String) async throws -> OperationResponse {
        let url = baseURL.appendingPathComponent("api/loops/\(id)/merge")
        return try await performPostRequestNoBody(url: url)
    }

    /// Discard a worktree loop (deletes worktree).
    func discardLoop(id: String) async throws -> OperationResponse {
        let url = baseURL.appendingPathComponent("api/loops/\(id)/discard")
        return try await performPostRequestNoBody(url: url)
    }

    // MARK: - Tasks API

    /// Fetch all tasks, optionally filtered by status.
    func getTasks(status: String? = nil) async throws -> TasksResponse {
        var url = baseURL.appendingPathComponent("api/tasks")
        if let status = status {
            url = url.appending(queryItems: [URLQueryItem(name: "status", value: status)])
        }
        return try await performRequest(url: url)
    }

    /// Create a new task.
    func createTask(title: String, description: String?, priority: UInt8 = 3, blockedBy: [String] = []) async throws -> TaskItem {
        let url = baseURL.appendingPathComponent("api/tasks")
        let body = CreateTaskRequest(title: title, description: description, priority: priority, blockedBy: blockedBy)
        return try await performPostRequest(url: url, body: body)
    }

    /// Update a task's status.
    func updateTask(id: String, status: String) async throws -> TaskItem {
        let url = baseURL.appendingPathComponent("api/tasks/\(id)")
        let body = UpdateTaskRequest(status: status)
        return try await performPutRequest(url: url, body: body)
    }

    // MARK: - Memories API

    /// Fetch the current memories content.
    func getMemories() async throws -> MemoriesContent {
        let url = baseURL.appendingPathComponent("api/memories")
        return try await performRequest(url: url)
    }

    /// Update memories content (overwrites file).
    func updateMemories(content: String) async throws -> MemoriesContent {
        let url = baseURL.appendingPathComponent("api/memories")
        let body = UpdateMemoriesRequest(content: content)
        return try await performPutRequest(url: url, body: body)
    }

    /// Export memories as a downloadable file.
    func exportMemories() async throws -> MemoriesExport {
        let url = baseURL.appendingPathComponent("api/memories/export")
        return try await performPostRequestNoBody(url: url)
    }

    // MARK: - RObot API

    /// Fetch pending human-in-the-loop questions.
    func getRobotQuestions() async throws -> [PendingQuestion] {
        let url = baseURL.appendingPathComponent("api/robot/questions")
        let response: QuestionsResponse = try await performRequest(url: url)
        return response.questions
    }

    /// Respond to a pending question.
    func respondToQuestion(questionId: String, responseText: String) async throws -> ResponseAck {
        let url = baseURL.appendingPathComponent("api/robot/response")
        let body = QuestionResponseRequest(questionId: questionId, responseText: responseText)
        return try await performPostRequest(url: url, body: body)
    }

    /// Send proactive guidance to a session.
    func sendGuidance(sessionId: String, guidanceText: String) async throws -> GuidanceAck {
        let url = baseURL.appendingPathComponent("api/robot/guidance")
        let body = GuidanceRequest(sessionId: sessionId, guidanceText: guidanceText)
        return try await performPostRequest(url: url, body: body)
    }

    // MARK: - Iterations API

    /// Fetch iteration history for a session.
    func getIterations(sessionId: String) async throws -> IterationsResponse {
        let url = baseURL.appendingPathComponent("api/sessions/\(sessionId)/iterations")
        return try await performRequest(url: url)
    }

    // MARK: - Merge Queue API

    /// Fetch the merge queue status.
    func getMergeQueue() async throws -> MergeQueueResponse {
        let url = baseURL.appendingPathComponent("api/merge-queue")
        return try await performRequest(url: url)
    }

    // MARK: - Hats API

    /// Fetch all available orchestration hats.
    func getHats() async throws -> [HatItem] {
        let url = baseURL.appendingPathComponent("api/hats")
        let response: HatsResponse = try await performRequest(url: url)
        return response.hats
    }

    // MARK: - Presets API

    /// Fetch all available configuration presets.
    func getPresets() async throws -> [PresetItem] {
        let url = baseURL.appendingPathComponent("api/presets")
        let response: PresetsResponse = try await performRequest(url: url)
        return response.presets
    }

    // MARK: - Config Export/Import API

    /// Export the current configuration as YAML.
    func exportConfig() async throws -> ExportConfigResponse {
        let url = baseURL.appendingPathComponent("api/config/export")
        return try await performPostRequestNoBody(url: url)
    }

    /// Import a configuration from YAML content.
    func importConfig(content: String) async throws -> ImportConfigResponse {
        let url = baseURL.appendingPathComponent("api/config/import")
        let body = ImportConfigRequest(content: content)
        return try await performPostRequest(url: url, body: body)
    }

    /// Generic GET request method with error handling.
    private func performRequest<T: Decodable>(url: URL) async throws -> T {
        var request = URLRequest(url: url)
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        return try await executeRequest(request)
    }

    /// Generic POST request method with JSON body.
    private func performPostRequest<T: Decodable, B: Encodable>(url: URL, body: B) async throws -> T {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        let encoder = JSONEncoder()
        request.httpBody = try encoder.encode(body)

        return try await executeRequest(request)
    }

    /// Generic POST request method without body.
    private func performPostRequestNoBody<T: Decodable>(url: URL) async throws -> T {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        return try await executeRequest(request)
    }

    /// Generic PUT request method with JSON body.
    private func performPutRequest<T: Decodable, B: Encodable>(url: URL, body: B) async throws -> T {
        var request = URLRequest(url: url)
        request.httpMethod = "PUT"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        let encoder = JSONEncoder()
        request.httpBody = try encoder.encode(body)

        return try await executeRequest(request)
    }

    /// Generic DELETE request method for resource removal.
    private func performDeleteRequest(url: URL) async throws {
        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        let _: EmptyResponse = try await executeRequest(request)
    }

    /// Execute HTTP request with common error handling.
    private func executeRequest<T: Decodable>(_ request: URLRequest) async throws -> T {
        let data: Data
        let response: URLResponse

        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw RalphError.networkError(error)
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            throw RalphError.networkError(URLError(.badServerResponse))
        }

        switch httpResponse.statusCode {
        case 200...299:
            break
        case 401:
            throw RalphError.unauthorized
        case 400:
            // Parse error response for validation errors
            if let errorBody = try? decoder.decode(ErrorResponse.self, from: data) {
                if errorBody.error.hasPrefix("invalid_path:") {
                    let path = String(errorBody.error.dropFirst("invalid_path:".count)).trimmingCharacters(in: .whitespaces)
                    throw RalphError.invalidPath(path)
                } else if errorBody.error.hasPrefix("invalid_topic:") {
                    let topic = String(errorBody.error.dropFirst("invalid_topic:".count)).trimmingCharacters(in: .whitespaces)
                    throw RalphError.invalidTopic(topic)
                }
            }
            throw RalphError.serverError(statusCode: 400)
        case 404:
            // Parse error response to determine specific error type
            if let errorBody = try? decoder.decode(ErrorResponse.self, from: data) {
                if errorBody.error.hasPrefix("config_not_found:") {
                    let path = String(errorBody.error.dropFirst("config_not_found:".count)).trimmingCharacters(in: .whitespaces)
                    throw RalphError.configNotFound(path)
                } else if errorBody.error.hasPrefix("prompt_not_found:") {
                    let path = String(errorBody.error.dropFirst("prompt_not_found:".count)).trimmingCharacters(in: .whitespaces)
                    throw RalphError.promptNotFound(path)
                }
            }
            throw RalphError.sessionNotFound
        case 500:
            // Parse error response for spawn failures
            if let errorBody = try? decoder.decode(ErrorResponse.self, from: data) {
                if errorBody.error.hasPrefix("failed_to_spawn:") {
                    let reason = String(errorBody.error.dropFirst("failed_to_spawn:".count)).trimmingCharacters(in: .whitespaces)
                    throw RalphError.spawnFailed(reason)
                }
            }
            throw RalphError.serverError(statusCode: 500)
        default:
            throw RalphError.serverError(statusCode: httpResponse.statusCode)
        }

        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            throw RalphError.decodingError(error)
        }
    }
}

/// Error response from the server.
private struct ErrorResponse: Decodable {
    let error: String
}

/// Empty response for endpoints that don't return data.
private struct EmptyResponse: Decodable {
    let status: String?

    init(from decoder: Decoder) throws {
        let container = try? decoder.container(keyedBy: CodingKeys.self)
        status = try container?.decodeIfPresent(String.self, forKey: .status)
    }

    enum CodingKeys: String, CodingKey {
        case status
    }
}
