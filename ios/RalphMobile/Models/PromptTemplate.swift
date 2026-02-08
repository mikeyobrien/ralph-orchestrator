import Foundation

/// A reusable prompt template for Ralph workflows.
/// Templates can be bundled (read-only) or custom (user-created, editable).
struct PromptTemplate: Codable, Identifiable, Hashable {
    let id: UUID
    var name: String
    var content: String
    var description: String
    var isBundled: Bool
    var createdAt: Date
    var updatedAt: Date

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case content
        case description
        case isBundled
        case createdAt
        case updatedAt
    }

    /// The 4 bundled prompt templates (read-only).
    static let bundled: [PromptTemplate] = [
        PromptTemplate(
            id: UUID(uuidString: "00000000-0000-0000-0001-000000000001")!,
            name: "Design Start",
            content: """
            # Design Task

            ## Objective
            [Describe what you want to design]

            ## Requirements
            - Functional requirement 1
            - Non-functional requirement 1

            ## Constraints
            - Time constraint
            - Technology constraint

            ## Success Criteria
            - Criterion 1
            - Criterion 2
            """,
            description: "Start a new design workflow",
            isBundled: true,
            createdAt: Date(timeIntervalSince1970: 0),
            updatedAt: Date(timeIntervalSince1970: 0)
        ),
        PromptTemplate(
            id: UUID(uuidString: "00000000-0000-0000-0001-000000000002")!,
            name: "Task Start",
            content: """
            # Implementation Task

            ## Goal
            [What should be implemented]

            ## Context
            [Relevant background information]

            ## Acceptance Criteria
            - [ ] Criterion 1
            - [ ] Criterion 2

            ## Notes
            [Any additional considerations]
            """,
            description: "Begin implementation task",
            isBundled: true,
            createdAt: Date(timeIntervalSince1970: 0),
            updatedAt: Date(timeIntervalSince1970: 0)
        ),
        PromptTemplate(
            id: UUID(uuidString: "00000000-0000-0000-0001-000000000003")!,
            name: "Code Review",
            content: """
            # Code Review Request

            ## Files to Review
            - `path/to/file.swift`

            ## Focus Areas
            - [ ] Code quality
            - [ ] Performance
            - [ ] Security

            ## Context
            [What does this code do]
            """,
            description: "Review code for quality",
            isBundled: true,
            createdAt: Date(timeIntervalSince1970: 0),
            updatedAt: Date(timeIntervalSince1970: 0)
        ),
        PromptTemplate(
            id: UUID(uuidString: "00000000-0000-0000-0001-000000000004")!,
            name: "Debug Issue",
            content: """
            # Debug Request

            ## Issue
            [Describe the bug]

            ## Expected Behavior
            [What should happen]

            ## Actual Behavior
            [What actually happens]

            ## Steps to Reproduce
            1. Step 1
            2. Step 2
            3. Step 3

            ## Relevant Logs
            ```
            [Paste logs here]
            ```
            """,
            description: "Debug and fix a problem",
            isBundled: true,
            createdAt: Date(timeIntervalSince1970: 0),
            updatedAt: Date(timeIntervalSince1970: 0)
        )
    ]
}

// MARK: - TemplateStore

/// Manages persistence of custom prompt templates using UserDefaults.
@MainActor
final class TemplateStore: ObservableObject {
    private let userDefaultsKey = "customPromptTemplates"

    @Published private(set) var customTemplates: [PromptTemplate] = []

    init() {
        loadCustomTemplates()
    }

    /// All templates: bundled + custom.
    var allTemplates: [PromptTemplate] {
        PromptTemplate.bundled + customTemplates
    }

    /// Load custom templates from UserDefaults.
    func loadCustomTemplates() {
        guard let data = UserDefaults.standard.data(forKey: userDefaultsKey) else {
            customTemplates = []
            return
        }
        do {
            customTemplates = try JSONDecoder().decode([PromptTemplate].self, from: data)
        } catch {
            #if DEBUG
            print("Failed to decode custom templates: \(error)")
            #endif
            customTemplates = []
        }
    }

    /// Save a new custom template or update an existing one.
    func saveCustom(_ template: PromptTemplate) {
        var templateToSave = template
        templateToSave.isBundled = false // Ensure custom templates are never bundled

        if let index = customTemplates.firstIndex(where: { $0.id == template.id }) {
            // Update existing
            templateToSave.updatedAt = Date()
            customTemplates[index] = templateToSave
        } else {
            // Add new
            customTemplates.append(templateToSave)
        }
        persistToUserDefaults()
    }

    /// Delete a custom template by ID. Bundled templates cannot be deleted.
    func deleteCustom(id: UUID) {
        // Check if it's a bundled template
        if PromptTemplate.bundled.contains(where: { $0.id == id }) {
            return // Cannot delete bundled templates
        }
        customTemplates.removeAll { $0.id == id }
        persistToUserDefaults()
    }

    /// Create a new custom template with initial values.
    func createCustomTemplate(name: String, content: String, description: String) -> PromptTemplate {
        let now = Date()
        let template = PromptTemplate(
            id: UUID(),
            name: name,
            content: content,
            description: description,
            isBundled: false,
            createdAt: now,
            updatedAt: now
        )
        saveCustom(template)
        return template
    }

    private func persistToUserDefaults() {
        do {
            let data = try JSONEncoder().encode(customTemplates)
            UserDefaults.standard.set(data, forKey: userDefaultsKey)
        } catch {
            #if DEBUG
            print("Failed to encode custom templates: \(error)")
            #endif
        }
    }
}
