import Foundation

/// ViewModel for managing parallel orchestration loops and the merge queue.
@MainActor
class LoopsViewModel: ObservableObject {
    @Published var loops: [LoopInfo] = []
    @Published var mergeQueue: MergeQueueResponse?
    @Published var isLoading = false
    @Published var error: String?
    @Published var operationResult: String?

    var primaryLoop: LoopInfo? { loops.first(where: { $0.status == "primary" }) }
    var worktreeLoops: [LoopInfo] { loops.filter { $0.status == "worktree" } }

    func fetchLoops() async {
        guard RalphAPIClient.isConfigured else {
            error = "API client not configured"
            return
        }

        isLoading = true
        error = nil

        do {
            loops = try await RalphAPIClient.shared.getLoops()
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func fetchMergeQueue() async {
        guard RalphAPIClient.isConfigured else { return }

        do {
            mergeQueue = try await RalphAPIClient.shared.getMergeQueue()
        } catch {
            self.error = error.localizedDescription
        }
    }

    func spawnLoop(prompt: String, configPath: String?) async {
        guard RalphAPIClient.isConfigured else { return }

        operationResult = nil
        do {
            let response = try await RalphAPIClient.shared.spawnLoop(
                prompt: prompt,
                configPath: configPath
            )
            operationResult = "Loop spawned successfully"
            await fetchLoops()
        } catch {
            self.error = error.localizedDescription
        }
    }

    func mergeLoop(id: String) async {
        guard RalphAPIClient.isConfigured else { return }

        operationResult = nil
        do {
            let response = try await RalphAPIClient.shared.mergeLoop(id: id)
            operationResult = "Loop merged"
            await fetchLoops()
            await fetchMergeQueue()
        } catch {
            self.error = error.localizedDescription
        }
    }

    func discardLoop(id: String) async {
        guard RalphAPIClient.isConfigured else { return }

        operationResult = nil
        do {
            let response = try await RalphAPIClient.shared.discardLoop(id: id)
            operationResult = "Loop discarded"
            await fetchLoops()
        } catch {
            self.error = error.localizedDescription
        }
    }
}
