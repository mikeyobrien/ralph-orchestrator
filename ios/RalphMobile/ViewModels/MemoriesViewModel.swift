import Foundation

/// ViewModel for managing Ralph's persistent memories (markdown content).
@MainActor
class MemoriesViewModel: ObservableObject {
    @Published var content: String = ""
    @Published var lastModified: String?
    @Published var isLoading = false
    @Published var isEditing = false
    @Published var editContent: String = ""
    @Published var error: String?
    @Published var operationResult: String?

    func fetchMemories() async {
        guard RalphAPIClient.isConfigured else {
            error = "API client not configured"
            return
        }

        isLoading = true
        error = nil

        do {
            let memories = try await RalphAPIClient.shared.getMemories()
            content = memories.content
            lastModified = memories.lastModified
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func startEditing() {
        editContent = content
        isEditing = true
    }

    func cancelEditing() {
        isEditing = false
        editContent = ""
    }

    func saveMemories() async {
        guard RalphAPIClient.isConfigured else { return }

        operationResult = nil
        do {
            let updated = try await RalphAPIClient.shared.updateMemories(content: editContent)
            content = updated.content
            lastModified = updated.lastModified
            isEditing = false
            editContent = ""
            operationResult = "Memories saved"
        } catch {
            self.error = error.localizedDescription
        }
    }

    func exportMemories() async -> MemoriesExport? {
        guard RalphAPIClient.isConfigured else { return nil }

        operationResult = nil
        do {
            let export = try await RalphAPIClient.shared.exportMemories()
            operationResult = "Memories exported to \(export.filename)"
            return export
        } catch {
            self.error = error.localizedDescription
            return nil
        }
    }
}
