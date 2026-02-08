import SwiftUI
import Combine

/// ViewModel for the markdown editor with debounced preview updates
@MainActor
final class EditorViewModel: ObservableObject {
    @Published var content: String = ""
    @Published var previewContent: String = ""
    @Published var isEditMode: Bool = true

    private var debounceTask: Task<Void, Never>?
    private let debounceDelay: UInt64 = 100_000_000 // 100ms in nanoseconds

    init(initialContent: String = "") {
        self.content = initialContent
        self.previewContent = initialContent
    }

    /// Update content with debounced preview refresh
    func updateContent(_ newContent: String) {
        content = newContent

        // Cancel existing debounce task
        debounceTask?.cancel()

        // Schedule new debounced preview update
        debounceTask = Task { [weak self] in
            guard let self else { return }
            do {
                try await Task.sleep(nanoseconds: debounceDelay)
                if !Task.isCancelled {
                    previewContent = newContent
                }
            } catch {
                // Task was cancelled, which is expected
            }
        }
    }

    /// Insert markdown syntax at the current position
    func insertMarkdown(_ syntax: MarkdownSyntax) -> String {
        switch syntax {
        case .bold:
            return "**bold**"
        case .italic:
            return "_italic_"
        case .code:
            return "`code`"
        case .header:
            return "# Header\n"
        case .quote:
            return "> Quote\n"
        case .link:
            return "[text](url)"
        case .list:
            return "- Item\n"
        case .codeBlock:
            return "```\ncode\n```\n"
        }
    }

    /// Toggle between edit and preview modes (for iPhone)
    func toggleMode() {
        isEditMode.toggle()
    }
}

/// Supported markdown syntax types
enum MarkdownSyntax {
    case bold
    case italic
    case code
    case header
    case quote
    case link
    case list
    case codeBlock
}
