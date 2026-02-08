import SwiftUI

/// Read-only catalog of orchestration hats (roles) Ralph can wear.
/// Fetches from GET /api/hats and displays emoji + name + description.
struct HatsListView: View {
    @State private var hats: [HatItem] = []
    @State private var isLoading: Bool = false
    @State private var errorMessage: String?

    var body: some View {
        Group {
            if isLoading {
                loadingView
            } else if let error = errorMessage {
                errorView(error)
            } else if hats.isEmpty {
                emptyView
            } else {
                hatsList
            }
        }
        .task {
            await loadHats()
        }
    }

    // MARK: - States

    private var loadingView: some View {
        VStack(spacing: 12) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
            Text("Loading hats...")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private func errorView(_ message: String) -> some View {
        VStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
                .font(.title)
                .foregroundColor(CyberpunkTheme.accentYellow)

            Text(message)
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)

            Button("Retry") {
                Task { await loadHats() }
            }
            .font(.caption.bold())
            .foregroundColor(CyberpunkTheme.accentCyan)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var emptyView: some View {
        VStack(spacing: 12) {
            Image(systemName: "theatermask.and.paintbrush")
                .font(.title)
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No hats found")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("Hats are discovered from preset files")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted.opacity(0.7))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    // MARK: - List

    private var hatsList: some View {
        ScrollView {
            LazyVStack(spacing: 8) {
                ForEach(hats) { hat in
                    HatCard(hat: hat)
                }
            }
            .padding()
        }
        .accessibilityIdentifier("hats-list-view")
    }

    // MARK: - Data

    private func loadHats() async {
        guard RalphAPIClient.isConfigured else {
            errorMessage = "API client not configured"
            return
        }

        isLoading = true
        errorMessage = nil

        do {
            hats = try await RalphAPIClient.shared.getHats()
            isLoading = false
        } catch {
            isLoading = false
            errorMessage = error.localizedDescription
        }
    }
}

// MARK: - Hat Card

private struct HatCard: View {
    let hat: HatItem

    var body: some View {
        HStack(spacing: 12) {
            Text(hat.emoji)
                .font(.title2)
                .frame(width: 40, height: 40)
                .background(CyberpunkTheme.bgHover)
                .cornerRadius(8)

            VStack(alignment: .leading, spacing: 4) {
                Text(hat.name)
                    .font(.system(.subheadline, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)

                if !hat.description.isEmpty {
                    Text(hat.description)
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .lineLimit(2)
                }
            }

            Spacer()
        }
        .padding(12)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
        .accessibilityIdentifier("hats-item-\(hat.name)")
    }
}
