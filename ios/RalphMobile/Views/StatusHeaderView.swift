import SwiftUI

/// Displays session status: iteration, hat, elapsed time, and connection indicator.
struct StatusHeaderView: View {
    let session: Session
    let connectionState: ConnectionState

    var body: some View {
        HStack {
            // Iteration counter
            Text("[iter \(session.iteration)/\(totalDisplay)]")
                .font(.system(.body, design: .monospaced))
                .accessibilityIdentifier("status-header-iteration")

            // Current hat
            Text(session.hat ?? "—")
                .font(.body)
                .accessibilityIdentifier("status-header-hat")

            // Elapsed time
            Text(formatElapsed(session.elapsedSeconds ?? 0))
                .font(.system(.body, design: .monospaced))
                .accessibilityIdentifier("status-header-elapsed")

            Spacer()

            // Connection indicator
            ConnectionIndicator(state: connectionState)
                .accessibilityIdentifier("status-header-connection")
        }
        .padding()
        .background(Color(.systemBackground))
        .accessibilityIdentifier("status-header")
    }

    private var totalDisplay: String {
        if let total = session.total {
            return "\(total)"
        }
        return "?"
    }

    private func formatElapsed(_ seconds: Int) -> String {
        let minutes = seconds / 60
        let secs = seconds % 60
        return String(format: "%02d:%02d", minutes, secs)
    }
}

/// Colored dot indicating connection state.
struct ConnectionIndicator: View {
    let state: ConnectionState

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(indicatorColor)
                .frame(width: 8, height: 8)
                .accessibilityIdentifier("connection-indicator-dot")

            if showText {
                Text(state.displayText)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .accessibilityIdentifier("connection-indicator-text")
            }
        }
    }

    private var indicatorColor: Color {
        switch state {
        case .connected:
            return .green
        case .connecting, .reconnecting:
            return .orange
        case .disconnected:
            return .gray
        case .error:
            return .red
        }
    }

    private var showText: Bool {
        switch state {
        case .connected:
            return false
        default:
            return true
        }
    }
}

#Preview {
    VStack {
        StatusHeaderView(
            session: Session(
                id: "test-123",
                iteration: 3,
                total: 10,
                hat: "🔨 Builder",
                elapsedSeconds: 142,
                mode: "live"
            ),
            connectionState: .connected
        )

        StatusHeaderView(
            session: Session(
                id: "test-456",
                iteration: 5,
                total: nil,
                hat: "📋 Planner",
                elapsedSeconds: 3723,
                mode: "live"
            ),
            connectionState: .reconnecting(attempt: 2)
        )

        StatusHeaderView(
            session: Session(
                id: "test-789",
                iteration: 1,
                total: 5,
                hat: nil,
                elapsedSeconds: 30,
                mode: "live"
            ),
            connectionState: .error("Connection lost")
        )
    }
}
