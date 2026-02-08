import SwiftUI

/// Detail view for displaying prompt file content.
///
/// Shows Markdown content in monospace font with cyberpunk dark theme,
/// loading/error states, and copy-to-clipboard functionality.
struct PromptDetailView: View {
    let prompt: Prompt
    let apiClient: RalphAPIClient

    @State private var content: String?
    @State private var isLoading = true
    @State private var error: Error?
    @State private var showCopyToast = false

    var body: some View {
        ZStack {
            // Cyberpunk dark background
            Color.black.ignoresSafeArea()

            if isLoading {
                loadingView
            } else if let error = error {
                errorView(error)
            } else if let content = content {
                contentView(content)
            }

            // Copy toast overlay
            if showCopyToast {
                copyToastView
            }
        }
        .navigationTitle(prompt.name)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    copyToClipboard()
                } label: {
                    Image(systemName: "doc.on.doc")
                }
                .disabled(content == nil)
            }
        }
        .task {
            await loadContent()
        }
    }

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: .pink))
                .scaleEffect(1.5)
            Text("Loading prompt...")
                .foregroundColor(.secondary)
        }
    }

    private func contentView(_ markdown: String) -> some View {
        ScrollView {
            Text(markdown)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(.pink)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func errorView(_ error: Error) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 48))
                .foregroundColor(.red)
            Text("Failed to load prompt")
                .font(.headline)
                .foregroundColor(.white)
            Text(error.localizedDescription)
                .font(.subheadline)
                .foregroundColor(.gray)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
            Button("Retry") {
                Task { await loadContent() }
            }
            .buttonStyle(.borderedProminent)
            .tint(.pink)
        }
    }

    private var copyToastView: some View {
        VStack {
            Spacer()
            Text("Copied to clipboard")
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(Color.pink.opacity(0.9))
                .foregroundColor(.black)
                .fontWeight(.medium)
                .cornerRadius(8)
                .padding(.bottom, 50)
        }
        .transition(.move(edge: .bottom).combined(with: .opacity))
    }

    private func loadContent() async {
        isLoading = true
        error = nil
        do {
            let response = try await apiClient.getPromptContent(path: prompt.path)
            content = response.content
        } catch {
            self.error = error
        }
        isLoading = false
    }

    private func copyToClipboard() {
        guard let content = content else { return }
        UIPasteboard.general.string = content
        withAnimation {
            showCopyToast = true
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            withAnimation {
                showCopyToast = false
            }
        }
    }
}
