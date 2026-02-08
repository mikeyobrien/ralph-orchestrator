import SwiftUI

/// View for reading, editing, and exporting Ralph's persistent memories.
struct MemoriesView: View {
    @StateObject private var viewModel = MemoriesViewModel()
    @State private var showExportSheet = false
    @State private var exportText: String = ""
    @State private var showSaveConfirmation = false

    var body: some View {
        VStack(spacing: 0) {
            headerView
            contentView
        }
        .background(CyberpunkTheme.bgPrimary)
        .task {
            await viewModel.fetchMemories()
        }
        .sheet(isPresented: $showExportSheet) {
            ShareSheet(text: exportText)
        }
        .confirmationDialog(
            "Save Memories?",
            isPresented: $showSaveConfirmation,
            titleVisibility: .visible
        ) {
            Button("Save", role: .destructive) {
                Task { await viewModel.saveMemories() }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This will overwrite the current memories content.")
        }
    }

    // MARK: - Header

    private var headerView: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("MEMORIES")
                    .font(.system(.headline, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)

                if let modified = viewModel.lastModified {
                    Text("Last modified: \(modified)")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }

            Spacer()

            HStack(spacing: 12) {
                // Export button
                Button {
                    Task {
                        if let export = await viewModel.exportMemories() {
                            exportText = export.content
                            showExportSheet = true
                        }
                    }
                } label: {
                    Image(systemName: "square.and.arrow.up")
                        .font(.body)
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }
                .accessibilityIdentifier("memories-button-export")

                // Edit/Save toggle
                if viewModel.isEditing {
                    Button {
                        showSaveConfirmation = true
                    } label: {
                        Text("Save")
                            .font(.system(.caption, design: .monospaced).bold())
                            .foregroundColor(CyberpunkTheme.accentGreen)
                    }
                    .accessibilityIdentifier("memories-button-save")

                    Button {
                        viewModel.cancelEditing()
                    } label: {
                        Text("Cancel")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.textMuted)
                    }
                } else {
                    Button {
                        viewModel.startEditing()
                    } label: {
                        Image(systemName: "pencil")
                            .font(.body)
                            .foregroundColor(CyberpunkTheme.textSecondary)
                    }
                    .accessibilityIdentifier("memories-button-edit")
                }
            }
        }
        .padding()
        .background(CyberpunkTheme.bgSecondary)
    }

    // MARK: - Content

    @ViewBuilder
    private var contentView: some View {
        if viewModel.isLoading {
            loadingView
        } else if let error = viewModel.error {
            errorView(error)
        } else if viewModel.isEditing {
            editView
        } else if viewModel.content.isEmpty {
            emptyView
        } else {
            readView
        }
    }

    private var readView: some View {
        ScrollView {
            Text(viewModel.content)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textPrimary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding()
        }
    }

    private var editView: some View {
        TextEditor(text: $viewModel.editContent)
            .font(.system(.body, design: .monospaced))
            .foregroundColor(CyberpunkTheme.textPrimary)
            .scrollContentBackground(.hidden)
            .background(CyberpunkTheme.bgPrimary)
            .padding()
    }

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)
            Text("Loading memories...")
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func errorView(_ message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.accentRed)
            Text(message)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)
            Button("Retry") {
                Task { await viewModel.fetchMemories() }
            }
            .font(.system(.body, design: .monospaced).bold())
            .foregroundColor(CyberpunkTheme.accentCyan)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyView: some View {
        VStack(spacing: 16) {
            Image(systemName: "brain")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.textMuted)
            Text("No memories yet")
                .font(.system(.headline, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
            Text("Ralph learns and remembers across sessions")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Share Sheet (UIActivityViewController wrapper)

struct ShareSheet: UIViewControllerRepresentable {
    let text: String

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: [text], applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}
