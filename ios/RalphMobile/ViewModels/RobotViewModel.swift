import Foundation

/// ViewModel for the RObot human-in-the-loop interface.
/// Polls for pending questions and allows sending guidance to running sessions.
@MainActor
class RobotViewModel: ObservableObject {
    @Published var questions: [PendingQuestion] = []
    @Published var isLoading = false
    @Published var error: String?
    @Published var operationResult: String?
    @Published var guidanceText: String = ""
    @Published var selectedSessionId: String?
    @Published var availableSessions: [Session] = []

    private var pollingTask: Task<Void, Never>?

    func fetchQuestions() async {
        guard RalphAPIClient.isConfigured else {
            error = "API client not configured"
            return
        }

        isLoading = true
        error = nil

        do {
            questions = try await RalphAPIClient.shared.getRobotQuestions()
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func fetchSessions() async {
        do {
            let sessions = try await RalphAPIClient.shared.getSessions()
            availableSessions = sessions
            // Auto-select if only one active session and none selected
            if selectedSessionId == nil {
                let activeSessions = sessions.filter { $0.status == "running" || $0.status == "paused" }
                if activeSessions.count == 1 {
                    selectedSessionId = activeSessions.first?.id
                }
            }
        } catch {
            // Silently fail — sessions are supplementary info
        }
    }

    func respondToQuestion(questionId: String, responseText: String) async {
        guard RalphAPIClient.isConfigured else { return }

        operationResult = nil
        do {
            _ = try await RalphAPIClient.shared.respondToQuestion(
                questionId: questionId,
                responseText: responseText
            )
            // Remove answered question from local list
            questions.removeAll { $0.id == questionId }
            operationResult = "Response sent"
        } catch {
            self.error = error.localizedDescription
        }
    }

    func sendGuidance() async {
        guard RalphAPIClient.isConfigured,
              let sessionId = selectedSessionId,
              !guidanceText.isEmpty else { return }

        operationResult = nil
        do {
            _ = try await RalphAPIClient.shared.sendGuidance(
                sessionId: sessionId,
                guidanceText: guidanceText
            )
            guidanceText = ""
            operationResult = "Guidance sent"
        } catch {
            self.error = error.localizedDescription
        }
    }

    func startPolling() {
        stopPolling()
        pollingTask = Task {
            await fetchSessions()
            while !Task.isCancelled {
                await fetchQuestions()
                try? await Task.sleep(nanoseconds: 3_000_000_000) // 3 seconds
            }
        }
    }

    func stopPolling() {
        pollingTask?.cancel()
        pollingTask = nil
    }
}
