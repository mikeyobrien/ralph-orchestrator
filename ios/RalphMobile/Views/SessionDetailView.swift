import SwiftUI

/// Main session monitoring view combining status header, metrics, and event feed.
struct SessionDetailView: View {
    @ObservedObject var viewModel: SessionViewModel
    @State private var showStopConfirmation = false
    @State private var showEmitSheet = false

    var body: some View {
        VStack(spacing: 0) {
            if let session = viewModel.currentSession {
                StatusHeaderView(
                    session: session,
                    connectionState: viewModel.connectionState
                )
                .accessibilityIdentifier("session-detail-status-indicator")

                // Token metrics section
                TokenMetricsView(metrics: viewModel.tokenMetrics)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)

                Divider()

                if viewModel.events.isEmpty {
                    emptyStateView
                } else {
                    EventFeedView(events: viewModel.events)
                }
            } else {
                noSessionView
            }
        }
        .navigationTitle("Session")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if let session = viewModel.currentSession {
                ToolbarItem(placement: .primaryAction) {
                    HStack(spacing: 16) {
                        Button {
                            showEmitSheet = true
                        } label: {
                            Label("Emit", systemImage: "paperplane")
                        }
                        .accessibilityIdentifier("session-detail-button-emit")

                        Button(role: .destructive) {
                            showStopConfirmation = true
                        } label: {
                            if viewModel.isStoppingSession {
                                ProgressView()
                            } else {
                                Label("Stop", systemImage: "stop.fill")
                            }
                        }
                        .disabled(viewModel.isStoppingSession)
                        .accessibilityIdentifier("session-detail-button-stop")
                    }
                }
            }
        }
        .sheet(isPresented: $showEmitSheet) {
            if let session = viewModel.currentSession {
                EventEmitSheet(
                    sessionId: session.id,
                    apiClient: viewModel.apiClient,
                    isPresented: $showEmitSheet
                )
            }
        }
        .confirmationDialog(
            "Stop Session",
            isPresented: $showStopConfirmation,
            titleVisibility: .visible
        ) {
            Button("Stop Session", role: .destructive) {
                guard let session = viewModel.currentSession else { return }
                Task {
                    await viewModel.stopSession(session)
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Are you sure you want to stop this session? This action cannot be undone.")
        }
        .alert("Error", isPresented: .init(
            get: { viewModel.stopSessionError != nil },
            set: { if !$0 { viewModel.stopSessionError = nil } }
        )) {
            Button("OK") {
                viewModel.stopSessionError = nil
            }
        } message: {
            if let error = viewModel.stopSessionError {
                Text(error)
            }
        }
    }

    private var emptyStateView: some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "tray")
                .font(.system(size: 48))
                .foregroundColor(.secondary)
            Text("No events yet")
                .font(.headline)
                .foregroundColor(.secondary)
            Text("Events will appear here as they stream in")
                .font(.subheadline)
                .foregroundColor(.secondary)
            Spacer()
        }
        .accessibilityIdentifier("session-detail-empty-state")
    }

    private var noSessionView: some View {
        VStack(spacing: 12) {
            Spacer()
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 48))
                .foregroundColor(.secondary)
            Text("No session selected")
                .font(.headline)
                .foregroundColor(.secondary)
            Spacer()
        }
        .accessibilityIdentifier("session-detail-no-session")
    }
}

#Preview("With Events") {
    let viewModel = SessionViewModel(
        baseURL: URL(string: "http://localhost:3000")!,
        apiKey: "test-key"
    )

    // Simulate connected state with session and events
    return NavigationStack {
        SessionDetailView(viewModel: viewModel)
    }
}

#Preview("Empty State") {
    let viewModel = SessionViewModel(
        baseURL: URL(string: "http://localhost:3000")!,
        apiKey: "test-key"
    )

    return NavigationStack {
        SessionDetailView(viewModel: viewModel)
    }
}
