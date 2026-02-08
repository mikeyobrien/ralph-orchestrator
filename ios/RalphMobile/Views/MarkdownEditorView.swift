import SwiftUI

/// Markdown editor with live preview - adaptive layout for iPhone/iPad
struct MarkdownEditorView: View {
    @StateObject private var viewModel: EditorViewModel
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @Environment(\.dismiss) private var dismiss
    @Binding var promptText: String

    /// Callback when user wants to insert a template
    var onTemplatesTap: (() -> Void)?

    /// Callback when user saves the prompt
    var onSave: ((String) -> Void)?

    /// State for AI improvement feature
    @State private var isAIEnabled: Bool = false
    @State private var showingAISheet: Bool = false
    @State private var aiLoadingState: AILoadingState = .idle
    @State private var suggestedPrompt: String = ""
    @State private var aiError: Error?

    /// AI loading states
    private enum AILoadingState {
        case idle
        case loading
        case success
        case error
    }

    init(promptText: Binding<String>, onTemplatesTap: (() -> Void)? = nil, onSave: ((String) -> Void)? = nil) {
        self._promptText = promptText
        self._viewModel = StateObject(wrappedValue: EditorViewModel(initialContent: promptText.wrappedValue))
        self.onTemplatesTap = onTemplatesTap
        self.onSave = onSave
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // Formatting toolbar
                toolbarView
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .background(CyberpunkTheme.bgCard)

                Divider()
                    .background(CyberpunkTheme.accentCyan.opacity(0.3))

                // Adaptive content area
                if horizontalSizeClass == .regular {
                    // iPad: Split-pane layout
                    splitPaneLayout
                } else {
                    // iPhone: Toggle between edit/preview
                    toggleLayout
                }

                Divider()
                    .background(CyberpunkTheme.accentCyan.opacity(0.3))

                // Bottom action bar
                bottomActionBar
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
                    .background(CyberpunkTheme.bgCard)
            }
            .background(CyberpunkTheme.bgPrimary)
            .navigationTitle("Markdown Editor")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        promptText = viewModel.content
                        onSave?(viewModel.content)
                        dismiss()
                    }
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .fontWeight(.semibold)
                }
            }
            .toolbarBackground(CyberpunkTheme.bgCard, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
        }
        .preferredColorScheme(.dark)
        .task {
            await checkAIEnabled()
        }
        .sheet(isPresented: $showingAISheet) {
            aiSheetContent
        }
    }

    // MARK: - AI Feature

    /// Check if Anthropic API key is configured
    private func checkAIEnabled() async {
        isAIEnabled = await KeychainManager.shared.exists(.anthropicAPIKey)
    }

    /// Trigger AI prompt improvement
    private func improvePromptWithAI() {
        aiLoadingState = .loading
        showingAISheet = true

        Task {
            do {
                guard let apiKey = await KeychainManager.shared.get(.anthropicAPIKey) else {
                    throw AnthropicError.noAPIKey
                }

                let improved = try await AnthropicClient.shared.improve(
                    prompt: viewModel.content,
                    apiKey: apiKey
                )

                await MainActor.run {
                    suggestedPrompt = improved
                    aiLoadingState = .success
                }
            } catch {
                await MainActor.run {
                    aiError = error
                    aiLoadingState = .error
                }
            }
        }
    }

    /// Content for the AI sheet based on loading state
    @ViewBuilder
    private var aiSheetContent: some View {
        switch aiLoadingState {
        case .idle:
            EmptyView()
        case .loading:
            AIImprovementLoadingView()
        case .success:
            AIImprovementSheet(
                originalPrompt: viewModel.content,
                suggestedPrompt: suggestedPrompt,
                onAccept: { accepted in
                    viewModel.updateContent(accepted)
                    resetAIState()
                },
                onReject: {
                    resetAIState()
                }
            )
        case .error:
            AIImprovementErrorView(
                error: aiError ?? AnthropicError.invalidResponse,
                onRetry: {
                    improvePromptWithAI()
                },
                onDismiss: {
                    resetAIState()
                }
            )
        }
    }

    /// Reset AI state after sheet dismissal
    private func resetAIState() {
        aiLoadingState = .idle
        suggestedPrompt = ""
        aiError = nil
        showingAISheet = false
    }

    // MARK: - Toolbar

    private var toolbarView: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                toolbarButton(icon: "bold", label: "Bold", syntax: .bold)
                    .accessibilityIdentifier("toolbar-bold")
                toolbarButton(icon: "italic", label: "Italic", syntax: .italic)
                    .accessibilityIdentifier("toolbar-italic")
                toolbarButton(icon: "chevron.left.forwardslash.chevron.right", label: "Code", syntax: .code)
                    .accessibilityIdentifier("toolbar-code")
                toolbarButton(icon: "number", label: "Header", syntax: .header)
                    .accessibilityIdentifier("toolbar-header")
                toolbarButton(icon: "text.quote", label: "Quote", syntax: .quote)
                    .accessibilityIdentifier("toolbar-quote")
                toolbarButton(icon: "link", label: "Link", syntax: .link)
                    .accessibilityIdentifier("toolbar-link")
                toolbarButton(icon: "list.bullet", label: "List", syntax: .list)
                    .accessibilityIdentifier("toolbar-list")

                Divider()
                    .frame(height: 24)
                    .background(CyberpunkTheme.accentCyan.opacity(0.3))
                    .padding(.horizontal, 4)

                // Templates button
                Button {
                    onTemplatesTap?()
                } label: {
                    Label("Templates", systemImage: "doc.on.doc")
                        .font(.system(size: 14, weight: .medium))
                        .foregroundColor(CyberpunkTheme.accentMagenta)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(CyberpunkTheme.accentMagenta.opacity(0.15))
                        .cornerRadius(8)
                }
                .accessibilityIdentifier("toolbar-templates")

                // AI Improve button (only shown when API key is configured)
                if isAIEnabled {
                    Button {
                        improvePromptWithAI()
                    } label: {
                        Label("AI Improve", systemImage: "sparkles")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(CyberpunkTheme.accentMagenta)
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            .background(CyberpunkTheme.accentMagenta.opacity(0.15))
                            .cornerRadius(8)
                    }
                    .accessibilityIdentifier("toolbar-ai-improve")
                }
            }
            .padding(.horizontal, 4)
        }
    }

    private func toolbarButton(icon: String, label: String, syntax: MarkdownSyntax) -> some View {
        Button {
            insertSyntax(syntax)
        } label: {
            Image(systemName: icon)
                .font(.system(size: 16, weight: .medium))
                .foregroundColor(CyberpunkTheme.accentCyan)
                .frame(width: 36, height: 36)
                .background(CyberpunkTheme.accentCyan.opacity(0.1))
                .cornerRadius(6)
        }
        .accessibilityLabel(label)
    }

    private func insertSyntax(_ syntax: MarkdownSyntax) {
        let insertion = viewModel.insertMarkdown(syntax)
        viewModel.updateContent(viewModel.content + insertion)
    }

    // MARK: - Split Pane Layout (iPad)

    private var splitPaneLayout: some View {
        GeometryReader { geometry in
            HStack(spacing: 1) {
                // Source editor pane
                sourceEditorPane
                    .frame(width: geometry.size.width * 0.5)
                    .accessibilityIdentifier("markdown-editor-source")

                Divider()
                    .background(CyberpunkTheme.accentCyan.opacity(0.5))

                // Preview pane
                previewPane
                    .frame(width: geometry.size.width * 0.5)
                    .accessibilityIdentifier("markdown-editor-preview")
            }
        }
    }

    // MARK: - Toggle Layout (iPhone)

    private var toggleLayout: some View {
        VStack(spacing: 0) {
            // Mode toggle
            Picker("Mode", selection: $viewModel.isEditMode) {
                Text("Edit").tag(true)
                Text("Preview").tag(false)
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .accessibilityIdentifier("markdown-editor-mode-toggle")

            // Show either editor or preview based on mode
            if viewModel.isEditMode {
                sourceEditorPane
                    .accessibilityIdentifier("markdown-editor-source")
            } else {
                previewPane
                    .accessibilityIdentifier("markdown-editor-preview")
            }
        }
    }

    // MARK: - Source Editor Pane

    private var sourceEditorPane: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("SOURCE")
                .font(.system(size: 11, weight: .semibold))
                .foregroundColor(CyberpunkTheme.accentCyan.opacity(0.7))
                .padding(.horizontal, 12)
                .padding(.vertical, 6)

            TextEditor(text: Binding(
                get: { viewModel.content },
                set: { viewModel.updateContent($0) }
            ))
            .font(.system(.body, design: .monospaced))
            .foregroundColor(.white)
            .scrollContentBackground(.hidden)
            .background(CyberpunkTheme.bgPrimary)
            .padding(.horizontal, 8)
        }
        .background(CyberpunkTheme.bgPrimary)
    }

    // MARK: - Preview Pane

    private var previewPane: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("PREVIEW")
                .font(.system(size: 11, weight: .semibold))
                .foregroundColor(CyberpunkTheme.accentMagenta.opacity(0.7))
                .padding(.horizontal, 12)
                .padding(.vertical, 6)

            ScrollView {
                markdownPreview
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .background(CyberpunkTheme.bgCard.opacity(0.5))
    }

    // MARK: - Markdown Preview Renderer

    @ViewBuilder
    private var markdownPreview: some View {
        if let attributedString = try? AttributedString(markdown: viewModel.previewContent, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            Text(attributedString)
                .font(.body)
                .foregroundColor(.white)
                .textSelection(.enabled)
        } else {
            // Fallback: plain text if markdown parsing fails
            Text(viewModel.previewContent)
                .font(.body)
                .foregroundColor(.white)
                .textSelection(.enabled)
        }
    }

    // MARK: - Bottom Action Bar

    private var bottomActionBar: some View {
        HStack {
            // Templates button (alternative access)
            Button {
                onTemplatesTap?()
            } label: {
                Label("Templates", systemImage: "doc.on.doc")
                    .font(.system(size: 14, weight: .medium))
            }
            .foregroundColor(CyberpunkTheme.accentMagenta)

            Spacer()

            // Character count
            Text("\(viewModel.content.count) chars")
                .font(.system(size: 12, design: .monospaced))
                .foregroundColor(.gray)

            Spacer()

            // Save button
            Button {
                promptText = viewModel.content
                onSave?(viewModel.content)
                dismiss()
            } label: {
                Label("Save", systemImage: "square.and.arrow.down")
                    .font(.system(size: 14, weight: .semibold))
            }
            .foregroundColor(CyberpunkTheme.accentGreen)
            .accessibilityIdentifier("toolbar-save")
        }
    }
}

// MARK: - Preview

#Preview("iPhone") {
    MarkdownEditorView(promptText: .constant("# Hello World\n\nThis is **bold** and _italic_ text.\n\n```\ncode block\n```"))
        .environment(\.horizontalSizeClass, .compact)
}

#Preview("iPad") {
    MarkdownEditorView(promptText: .constant("# Hello World\n\nThis is **bold** and _italic_ text.\n\n```\ncode block\n```"))
        .environment(\.horizontalSizeClass, .regular)
}
