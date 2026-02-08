import Foundation

/// ViewModel for managing session state and coordinating API/SSE services.
@MainActor
class SessionViewModel: ObservableObject {
    @Published var sessions: [Session] = []
    @Published var currentSession: Session?
    @Published var events: [Event] = []
    @Published var connectionState: ConnectionState = .disconnected
    @Published var configs: [Config] = []
    @Published var prompts: [Prompt] = []
    @Published var isStartingRun: Bool = false
    @Published var startRunError: String?
    @Published var isStoppingSession: Bool = false
    @Published var stopSessionError: String?
    @Published var tokenMetrics: TokenMetrics = TokenMetrics()
    @Published var backpressure: BackpressureStatus?
    @Published var scratchpadContent: String?
    @Published var configContent: String?
    @Published var promptContent: String?

    private(set) var apiClient: RalphAPIClient
    private var eventStreamService: EventStreamService
    private let reconnectionManager = ReconnectionManager()
    private var eventStreamTask: Task<Void, Never>?

    init(baseURL: URL, apiKey: String) {
        self.apiClient = RalphAPIClient(baseURL: baseURL, apiKey: apiKey)
        self.eventStreamService = EventStreamService(baseURL: baseURL, apiKey: apiKey)
    }

    /// Update the API client with new credentials (called when settings change).
    func updateClient(baseURL: URL, apiKey: String) {
        self.apiClient = RalphAPIClient(baseURL: baseURL, apiKey: apiKey)
        self.eventStreamService = EventStreamService(baseURL: baseURL, apiKey: apiKey)
    }

    /// Fetch all available sessions from the server.
    func fetchSessions() async {
        do {
            sessions = try await apiClient.getSessions()
        } catch let error as RalphError {
            handleError(error, attemptReconnect: false)
        } catch {
            connectionState = .error(error.localizedDescription)
        }
    }

    /// Fetch status for a specific session.
    func fetchSessionStatus(id: String) async {
        do {
            currentSession = try await apiClient.getSessionStatus(id: id)
        } catch let error as RalphError {
            handleError(error, attemptReconnect: false)
        } catch {
            connectionState = .error(error.localizedDescription)
        }
    }

    /// Fetch all available configuration presets.
    func loadConfigs() async {
        do {
            configs = try await apiClient.getConfigs()
        } catch let error as RalphError {
            handleError(error, attemptReconnect: false)
        } catch {
            connectionState = .error(error.localizedDescription)
        }
    }

    /// Fetch all available prompt files.
    func loadPrompts() async {
        do {
            prompts = try await apiClient.getPrompts()
        } catch let error as RalphError {
            handleError(error, attemptReconnect: false)
        } catch {
            connectionState = .error(error.localizedDescription)
        }
    }

    /// Start a new Ralph run with the given config and prompt.
    func startRun(config: Config, prompt: Prompt) async {
        isStartingRun = true
        startRunError = nil

        do {
            let response = try await apiClient.startSession(
                config: config.path,
                prompt: prompt.path,
                workingDir: nil
            )

            // Refresh sessions list to pick up the new session
            await fetchSessions()

            // Find and connect to the newly created session
            if let newSession = sessions.first(where: { $0.id == response.id }) {
                connect(to: newSession)
            }

            isStartingRun = false
        } catch let error as RalphError {
            isStartingRun = false
            startRunError = error.localizedDescription
        } catch {
            isStartingRun = false
            startRunError = error.localizedDescription
        }
    }

    /// Stop a running session.
    func stopSession(_ session: Session) async {
        isStoppingSession = true
        stopSessionError = nil

        do {
            try await apiClient.stopSession(id: session.id)

            // Disconnect from event stream
            disconnect()

            // Refresh sessions list
            await fetchSessions()

            isStoppingSession = false
        } catch let error as RalphError {
            isStoppingSession = false
            stopSessionError = error.localizedDescription
        } catch {
            isStoppingSession = false
            stopSessionError = error.localizedDescription
        }
    }

    /// Connect to the SSE event stream for a session.
    func connect(to session: Session) {
        // Cancel any existing stream
        eventStreamTask?.cancel()

        currentSession = session
        events = []
        tokenMetrics.reset()
        reconnectionManager.reset()

        // Fetch full session status first, then decide whether to start SSE
        Task { [weak self] in
            guard let self else { return }
            await fetchSessionStatus(id: session.id)
            // Only start SSE stream for active (running/paused) sessions
            if isSessionActive {
                connectionState = .connecting
                startEventStream(for: session)
            } else {
                connectionState = .disconnected
            }
        }
    }

    /// Disconnect from the current event stream.
    func disconnect() {
        eventStreamTask?.cancel()
        eventStreamTask = nil
        reconnectionManager.reset()
        connectionState = .disconnected
    }

    /// Start the event stream with automatic reconnection on failure.
    private func startEventStream(for session: Session) {
        eventStreamTask = Task { [weak self] in
            guard let self else { return }
            do {
                #if DEBUG
                debugLog("[SessionViewModel] Connecting to session: \(session.id)")
                #endif
                let stream = await eventStreamService.connect(sessionId: session.id)
                connectionState = .connected
                reconnectionManager.reset()
                #if DEBUG
                debugLog("[SessionViewModel] Connected, waiting for events...")
                #endif

                for try await event in stream {
                    #if DEBUG
                    debugLog("[SessionViewModel] Received event: \(event.topic ?? event.type)")
                    #endif
                    // CRITICAL FIX: Explicitly notify SwiftUI before array mutation
                    // @Published in async streams doesn't always trigger view updates
                    self.objectWillChange.send()
                    self.events.insert(event, at: 0)  // Newest first
                    self.processEventForMetrics(event)
                    #if DEBUG
                    debugLog("[SessionViewModel] Events count after insert: \(self.events.count)")
                    #endif
                }

                // Stream ended normally - attempt reconnect
                scheduleReconnect(for: session)
            } catch is CancellationError {
                // Task was cancelled, don't reconnect
                return
            } catch let error as RalphError {
                handleError(error, attemptReconnect: true, session: session)
            } catch {
                connectionState = .error(error.localizedDescription)
                scheduleReconnect(for: session)
            }
        }
    }

    /// Schedule a reconnection attempt with exponential backoff.
    private func scheduleReconnect(for session: Session) {
        guard let delay = reconnectionManager.nextDelay() else {
            connectionState = .error("Max reconnection attempts exceeded")
            return
        }

        let attempt = reconnectionManager.currentAttempt
        connectionState = .reconnecting(attempt: attempt)

        eventStreamTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))

            guard !Task.isCancelled, let self else { return }

            connectionState = .connecting
            startEventStream(for: session)
        }
    }

    /// Process an event for token metrics aggregation.
    private func processEventForMetrics(_ event: Event) {
        // Parse Assistant events for usage data
        if event.topic == "assistant" {
            if let usage = TokenMetricsParser.parseUsage(from: event.payload) {
                tokenMetrics.addUsage(input: usage.inputTokens, output: usage.outputTokens)
                #if DEBUG
                debugLog("[SessionViewModel] Token metrics updated: \(tokenMetrics.totalTokens) total")
                #endif
            }
        }

        // Parse Result events for final cost and duration
        if event.topic == "result" {
            if let result = TokenMetricsParser.parseResult(from: event.payload) {
                if let cost = result.totalCostUsd {
                    tokenMetrics.estimatedCost = cost
                }
                if let duration = result.durationMs {
                    tokenMetrics.durationMs = duration
                }
                #if DEBUG
                debugLog("[SessionViewModel] Result metrics: cost=\(String(describing: tokenMetrics.estimatedCost)), duration=\(String(describing: tokenMetrics.durationMs))")
                #endif
            }
        }
    }

    /// Handle API/stream errors with optional reconnection.
    private func handleError(_ error: RalphError, attemptReconnect: Bool, session: Session? = nil) {
        switch error {
        case .unauthorized:
            connectionState = .error("Invalid API key")
        case .sessionNotFound:
            connectionState = .error("Session not found")
        case .networkError(let underlying):
            connectionState = .error("Network: \(underlying.localizedDescription)")
            if attemptReconnect, let session = session {
                scheduleReconnect(for: session)
            }
        case .decodingError(let underlying):
            connectionState = .error("Decode: \(underlying.localizedDescription)")
        case .invalidURL:
            connectionState = .error("Invalid server URL")
        case .serverError(let code):
            connectionState = .error("Server error: \(code)")
            // Reconnect on server errors (server might be restarting)
            if attemptReconnect, let session = session {
                scheduleReconnect(for: session)
            }
        case .configNotFound(let path):
            connectionState = .error("Config not found: \(path)")
        case .promptNotFound(let path):
            connectionState = .error("Prompt not found: \(path)")
        case .spawnFailed(let reason):
            connectionState = .error("Failed to start: \(reason)")
        case .invalidPath(let path):
            connectionState = .error("Invalid path: \(path)")
        case .invalidTopic(let topic):
            connectionState = .error("Invalid topic: \(topic)")
        }
    }

    // MARK: - Route Control Methods

    /// Start a new run (uses first available config/prompt if none specified)
    func startRun() async {
        guard let config = configs.first, let prompt = prompts.first else {
            startRunError = "No configs or prompts available"
            return
        }
        await startRun(config: config, prompt: prompt)
    }

    /// Pause the current running session.
    func pauseRun() async {
        guard let session = currentSession else { return }
        do {
            try await apiClient.pauseSession(id: session.id)
            currentSession?.status = "paused"
        } catch {
            connectionState = .error("Failed to pause: \(error.localizedDescription)")
        }
    }

    /// Resume a paused session.
    func resumeRun() async {
        guard let session = currentSession else { return }
        do {
            try await apiClient.resumeSession(id: session.id)
            currentSession?.status = "running"
        } catch {
            connectionState = .error("Failed to resume: \(error.localizedDescription)")
        }
    }

    /// Stop the current running session.
    func stopRun() async {
        guard let session = currentSession else { return }
        await stopSession(session)
    }

    /// Send a steering message to the current session.
    func sendSteeringMessage(_ message: String) async {
        guard let session = currentSession else { return }
        do {
            try await apiClient.steerSession(id: session.id, message: message)
            // Add steering event to local events
            let steerEvent = Event(
                timestamp: Date(),
                topic: "user.steer",
                payload: message,
                hat: "user",
                type: "user.steer"
            )
            events.insert(steerEvent, at: 0)
        } catch {
            connectionState = .error("Failed to send steering: \(error.localizedDescription)")
        }
    }

    /// Clear all events from the current view.
    func clearEvents() {
        events.removeAll()
    }

    /// Refresh session data (status, events, etc.).
    func refreshSession() async {
        guard let session = currentSession else { return }
        await fetchSessionStatus(id: session.id)
        await fetchScratchpad()
    }

    /// Restart a completed session.
    func restartRun() async {
        guard let session = currentSession else { return }
        // Clear state
        events.removeAll()
        tokenMetrics.reset()
        // Re-start would typically create new session with same config
        // For now, just refresh state
        await fetchSessionStatus(id: session.id)
    }

    /// Emit a signal to the current session.
    func emitSignal(type: String, message: String) async {
        guard let session = currentSession else { return }
        do {
            _ = try await apiClient.emitEvent(sessionId: session.id, topic: type, payload: message)
            // Add signal event to local events
            let signalEvent = Event(
                timestamp: Date(),
                topic: type,
                payload: message,
                hat: "user",
                type: type
            )
            events.insert(signalEvent, at: 0)
        } catch {
            connectionState = .error("Failed to emit signal: \(error.localizedDescription)")
        }
    }

    // MARK: - Content Fetching

    /// Fetch scratchpad content for the current session.
    func fetchScratchpad() async {
        guard let session = currentSession else { return }
        do {
            scratchpadContent = try await apiClient.getScratchpad(sessionId: session.id)
        } catch {
            #if DEBUG
            debugLog("[SessionViewModel] Failed to fetch scratchpad: \(error)")
            #endif
        }
    }

    /// Fetch config content.
    func fetchConfig() async {
        guard let config = configs.first else { return }
        do {
            let response = try await apiClient.getConfigContent(path: config.path)
            configContent = response.content
        } catch {
            #if DEBUG
            debugLog("[SessionViewModel] Failed to fetch config: \(error)")
            #endif
        }
    }

    /// Fetch prompt content.
    func fetchPrompt() async {
        guard let prompt = prompts.first else { return }
        do {
            let response = try await apiClient.getPromptContent(path: prompt.path)
            promptContent = response.content
        } catch {
            #if DEBUG
            debugLog("[SessionViewModel] Failed to fetch prompt: \(error)")
            #endif
        }
    }

    var isLoading: Bool {
        isStartingRun || isStoppingSession
    }

    /// Whether the current session is actively running or paused (i.e., interactive).
    /// Completed/stopped/idle sessions are NOT active — they should be read-only.
    var isSessionActive: Bool {
        guard let session = currentSession else { return false }
        // Backend-provided liveness (always present after fetchSessionStatus)
        if session.mode == "live" { return true }
        // iOS-only local state (set by pauseRun/resumeRun)
        if let status = session.status {
            return status == "running" || status == "paused"
        }
        return false
    }
}
