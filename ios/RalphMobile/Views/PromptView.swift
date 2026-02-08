import SwiftUI

/// Prompt Editor screen - displays and edits PROMPT.md with markdown preview
/// Matches PromptView from ralph-mobile-ui-documentation.md
struct PromptView: View {
    @ObservedObject var viewModel: SessionViewModel
    @State private var editedContent: String = ""
    @State private var isEditing: Bool = false
    @State private var showPreview: Bool = false
    @State private var hasChanges: Bool = false

    var body: some View {
        VStack(spacing: 0) {
            // Header
            headerSection

            // Mode toggle
            modeToggle

            // Content area
            if viewModel.isLoading {
                loadingView
            } else if let content = viewModel.promptContent {
                if showPreview {
                    previewView(content: isEditing ? editedContent : content)
                } else {
                    editorView(content: isEditing ? editedContent : content)
                }
            } else {
                emptyPromptView
            }
        }
        .background(CyberpunkTheme.bgPrimary)
        .task {
            await viewModel.fetchPrompt()
        }
        .onChange(of: viewModel.promptContent) { newValue in
            if let content = newValue, !isEditing {
                editedContent = content
            }
        }
    }

    // MARK: - Header Section

    private var headerSection: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("PROMPT")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .kerning(2)

                HStack(spacing: 8) {
                    Image(systemName: "pencil.circle.fill")
                        .foregroundColor(CyberpunkTheme.accentPurple)
                    Text("PROMPT.md")
                        .font(.title2.bold())
                        .foregroundColor(CyberpunkTheme.textPrimary)
                }
            }

            Spacer()

            // Action buttons
            HStack(spacing: 12) {
                if isEditing && hasChanges {
                    Button {
                        // Save changes
                        Task {
                            // TODO: Implement save API
                            isEditing = false
                            hasChanges = false
                        }
                    } label: {
                        Text("SAVE")
                            .font(.system(.caption, design: .monospaced).bold())
                            .foregroundColor(CyberpunkTheme.bgPrimary)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            .background(CyberpunkTheme.accentGreen)
                            .cornerRadius(6)
                    }
                    .accessibilityIdentifier("prompt-button-save")
                }

                Button {
                    if isEditing {
                        // Cancel editing
                        editedContent = viewModel.promptContent ?? ""
                        isEditing = false
                        hasChanges = false
                    } else {
                        isEditing = true
                        editedContent = viewModel.promptContent ?? ""
                    }
                } label: {
                    Text(isEditing ? "CANCEL" : "EDIT")
                        .font(.system(.caption, design: .monospaced).bold())
                        .foregroundColor(isEditing ? CyberpunkTheme.accentRed : CyberpunkTheme.accentCyan)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(CyberpunkTheme.bgTertiary)
                        .cornerRadius(6)
                }
                .accessibilityIdentifier(isEditing ? "prompt-button-cancel" : "prompt-button-edit")
            }
        }
        .padding()
    }

    // MARK: - Mode Toggle

    private var modeToggle: some View {
        HStack(spacing: 0) {
            Button {
                showPreview = false
            } label: {
                Text("EDIT")
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(showPreview ? CyberpunkTheme.textMuted : CyberpunkTheme.accentCyan)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .background(showPreview ? Color.clear : CyberpunkTheme.bgTertiary)
            }

            Button {
                showPreview = true
            } label: {
                Text("PREVIEW")
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(showPreview ? CyberpunkTheme.accentCyan : CyberpunkTheme.textMuted)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .background(showPreview ? CyberpunkTheme.bgTertiary : Color.clear)
            }
        }
        .background(CyberpunkTheme.bgSecondary)
        .overlay(
            Rectangle()
                .frame(height: 1)
                .foregroundColor(CyberpunkTheme.border),
            alignment: .bottom
        )
        .accessibilityIdentifier("prompt-toggle-mode")
    }

    // MARK: - Editor View

    private func editorView(content: String) -> some View {
        ScrollView {
            if isEditing {
                TextEditor(text: $editedContent)
                    .font(.system(.body, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textPrimary)
                    .scrollContentBackground(.hidden)
                    .frame(minHeight: 400)
                    .onChange(of: editedContent) { _ in
                        hasChanges = editedContent != viewModel.promptContent
                    }
            } else {
                Text(content)
                    .font(.system(.body, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textPrimary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .accessibilityIdentifier("prompt-content-view")
    }

    // MARK: - Preview View

    private func previewView(content: String) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // Parse and render markdown
                MarkdownRenderer(content: content)
            }
            .padding()
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(CyberpunkTheme.bgCard)
        .accessibilityIdentifier("prompt-preview-view")
    }

    // MARK: - Loading & Empty States

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)

            Text("Loading prompt...")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyPromptView: some View {
        VStack(spacing: 12) {
            Image(systemName: "doc.text")
                .font(.system(size: 40))
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No prompt loaded")
                .font(.headline)
                .foregroundColor(CyberpunkTheme.textSecondary)

            Text("Start a session to view its prompt")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)

            Button {
                Task { await viewModel.fetchPrompt() }
            } label: {
                Text("Retry")
                    .font(.subheadline.bold())
                    .foregroundColor(CyberpunkTheme.bgPrimary)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 10)
                    .background(CyberpunkTheme.accentCyan)
                    .cornerRadius(8)
            }
            .padding(.top, 8)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Markdown Renderer

private struct MarkdownRenderer: View {
    let content: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(parseLines(), id: \.id) { line in
                renderLine(line)
            }
        }
    }

    private func parseLines() -> [MarkdownLine] {
        content.split(separator: "\n", omittingEmptySubsequences: false).enumerated().map { index, line in
            MarkdownLine(id: index, content: String(line))
        }
    }

    @ViewBuilder
    private func renderLine(_ line: MarkdownLine) -> some View {
        let content = line.content

        if content.hasPrefix("# ") {
            // H1
            Text(String(content.dropFirst(2)))
                .font(.title.bold())
                .foregroundColor(CyberpunkTheme.textPrimary)
                .padding(.top, 8)
        } else if content.hasPrefix("## ") {
            // H2
            Text(String(content.dropFirst(3)))
                .font(.title2.bold())
                .foregroundColor(CyberpunkTheme.accentCyan)
                .padding(.top, 6)
        } else if content.hasPrefix("### ") {
            // H3
            Text(String(content.dropFirst(4)))
                .font(.title3.bold())
                .foregroundColor(CyberpunkTheme.accentMagenta)
                .padding(.top, 4)
        } else if content.hasPrefix("- ") || content.hasPrefix("* ") {
            // List item
            HStack(alignment: .top, spacing: 8) {
                Text("•")
                    .foregroundColor(CyberpunkTheme.accentCyan)
                Text(String(content.dropFirst(2)))
                    .foregroundColor(CyberpunkTheme.textPrimary)
            }
        } else if content.hasPrefix("```") {
            // Code block marker
            EmptyView()
        } else if content.isEmpty {
            // Empty line
            Spacer()
                .frame(height: 8)
        } else {
            // Regular paragraph
            Text(content)
                .foregroundColor(CyberpunkTheme.textSecondary)
        }
    }
}

private struct MarkdownLine: Identifiable {
    let id: Int
    let content: String
}

#Preview {
    NavigationStack {
        PromptView(
            viewModel: SessionViewModel(
                baseURL: URL(string: "http://localhost:8080")!,
                apiKey: ""
            )
        )
    }
}
