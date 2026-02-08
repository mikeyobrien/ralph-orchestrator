import SwiftUI

/// Main Dashboard view showing hat status, controls, backpressure, and recent activity
/// Matches DashboardView from ralph-mobile-prototype.jsx
struct DashboardView: View {
    @ObservedObject var viewModel: SessionViewModel
    @Binding var showSteeringSheet: Bool

    // Current session state
    private var currentHat: String {
        viewModel.currentSession?.hat ?? "idle"
    }

    private var iteration: Int {
        viewModel.currentSession?.iteration ?? 0
    }

    private var runState: ControlBar.RunState {
        switch viewModel.currentSession?.status {
        case "running": return .running
        case "paused": return .paused
        default: return .idle
        }
    }

    // Recent tool calls (last 5)
    private var recentToolCalls: [Event] {
        Array(viewModel.events
            .filter { $0.type == "tool.call" }
            .suffix(5)
            .reversed())
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                // Header with connection status
                headerSection

                // Hat Status Card
                hatStatusSection

                // Control Bar
                controlSection

                // Backpressure Status
                backpressureSection

                // Stats Grid
                statsGridSection

                // Recent Tool Calls
                recentToolsSection
            }
            .padding()
        }
        .background(CyberpunkTheme.bgPrimary)
    }

    // MARK: - Header Section

    private var headerSection: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("RALPH ORCHESTRATOR")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .kerning(2)

                Text("Dashboard")
                    .font(.title2.bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)
            }

            Spacer()

            // Connection status indicator
            HStack(spacing: 8) {
                Circle()
                    .fill(connectionColor)
                    .frame(width: 8, height: 8)
                    .shadow(color: connectionColor.opacity(0.5), radius: 4)

                Text(connectionText)
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textSecondary)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(CyberpunkTheme.bgTertiary)
            .cornerRadius(16)
            .accessibilityIdentifier("dashboard-status-indicator")
        }
    }

    private var connectionColor: Color {
        switch viewModel.connectionState {
        case .connected: return CyberpunkTheme.accentGreen
        case .connecting, .reconnecting: return CyberpunkTheme.accentYellow
        case .disconnected: return CyberpunkTheme.textMuted
        case .error: return CyberpunkTheme.accentRed
        }
    }

    private var connectionText: String {
        switch viewModel.connectionState {
        case .connected: return "Connected"
        case .connecting: return "Connecting..."
        case .reconnecting(let attempt): return "Reconnecting (\(attempt))..."
        case .disconnected: return "Disconnected"
        case .error: return "Error"
        }
    }

    // MARK: - Hat Status Section

    private var hatStatusSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("ACTIVE HAT")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .kerning(1)

            HatStatusCard(
                hatName: currentHat,
                iteration: iteration,
                triggerEvent: viewModel.currentSession?.triggerEvent,
                publishedEvents: viewModel.currentSession?.availablePublishes ?? []
            )
            .accessibilityIdentifier("dashboard-hat-status-card")
        }
        .accessibilityIdentifier("dashboard-hat-section")
    }

    // MARK: - Control Section

    private var controlSection: some View {
        VStack(spacing: 12) {
            ControlBar(
                state: runState,
                onStart: { Task { await viewModel.startRun() } },
                onPause: { Task { await viewModel.pauseRun() } },
                onResume: { Task { await viewModel.resumeRun() } },
                onStop: { Task { await viewModel.stopRun() } }
            )

            // Elapsed time
            if let startTime = viewModel.currentSession?.startTime {
                Text(elapsedTimeString(from: startTime))
                    .font(.system(.title3, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
            }
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
        .accessibilityIdentifier("dashboard-controls")
    }

    private func elapsedTimeString(from startTime: Date) -> String {
        let elapsed = Date().timeIntervalSince(startTime)
        let minutes = Int(elapsed) / 60
        let seconds = Int(elapsed) % 60
        return String(format: "%02d:%02d", minutes, seconds)
    }

    // MARK: - Backpressure Section

    private var backpressureSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("BACKPRESSURE")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .kerning(1)

            BackpressureStatusView(checks: [
                .init(name: "tests", passed: viewModel.backpressure?.testsPass ?? false),
                .init(name: "lint", passed: viewModel.backpressure?.lintPass ?? false),
                .init(name: "typecheck", passed: viewModel.backpressure?.typecheckPass ?? false)
            ])
            .accessibilityIdentifier("dashboard-backpressure-status")
        }
        .accessibilityIdentifier("dashboard-backpressure-section")
    }

    // MARK: - Stats Grid Section

    private var statsGridSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("STATISTICS")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .kerning(1)

            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible())
            ], spacing: 12) {
                StatCard(
                    title: "Iterations",
                    value: "\(iteration)",
                    icon: "repeat",
                    color: CyberpunkTheme.accentCyan
                )
                .accessibilityIdentifier("dashboard-stat-iterations")

                StatCard(
                    title: "Tool Calls",
                    value: "\(viewModel.events.filter { $0.type == "tool.call" }.count)",
                    icon: "wrench",
                    color: CyberpunkTheme.accentMagenta
                )
                .accessibilityIdentifier("dashboard-stat-tool-calls")

                StatCard(
                    title: "Events",
                    value: "\(viewModel.events.count)",
                    icon: "bolt",
                    color: CyberpunkTheme.accentYellow
                )
                .accessibilityIdentifier("dashboard-stat-events")

                StatCard(
                    title: "Tokens",
                    value: formatTokens(viewModel.tokenMetrics.totalTokens),
                    icon: "textformat",
                    color: CyberpunkTheme.accentGreen
                )
                .accessibilityIdentifier("dashboard-stat-tokens")
            }
        }
        .accessibilityIdentifier("dashboard-stats-grid")
    }

    private func formatTokens(_ count: Int) -> String {
        if count >= 1000 {
            return String(format: "%.1fK", Double(count) / 1000)
        }
        return "\(count)"
    }

    // MARK: - Recent Tools Section

    private var recentToolsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("RECENT TOOL CALLS")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .kerning(1)

                Spacer()

                NavigationLink(destination: StreamView(viewModel: viewModel)) {
                    Text("View All")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.accentCyan)
                }
            }

            if recentToolCalls.isEmpty {
                emptyToolCallsView
            } else {
                ForEach(recentToolCalls) { event in
                    ToolCallCard(
                        toolName: event.toolName ?? "unknown",
                        status: statusFromEvent(event),
                        input: event.input,
                        output: event.output,
                        duration: event.duration,
                        timestamp: event.timestamp
                    )
                }
            }
        }
    }

    private var emptyToolCallsView: some View {
        VStack(spacing: 8) {
            Image(systemName: "wrench.and.screwdriver")
                .font(.system(size: 32))
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No tool calls yet")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textSecondary)

            Text("Start a route to see tool activity")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 32)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .accessibilityIdentifier("dashboard-empty-tools")
    }

    private func statusFromEvent(_ event: Event) -> StatusIndicator.ExecutionStatus {
        switch event.status?.lowercased() {
        case "running": return .running
        case "completed": return .completed
        case "pending": return .pending
        case "error", "failed": return .error
        default: return .completed
        }
    }
}

// MARK: - Stat Card

private struct StatCard: View {
    let title: String
    let value: String
    let icon: String
    let color: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: icon)
                    .font(.caption)
                    .foregroundColor(color)

                Spacer()
            }

            Text(value)
                .font(.system(.title2, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textPrimary)

            Text(title)
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textSecondary)
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(color.opacity(0.3), lineWidth: 1)
        )
    }
}

#Preview {
    NavigationStack {
        DashboardView(
            viewModel: SessionViewModel(
                baseURL: URL(string: "http://localhost:8080")!,
                apiKey: ""
            ),
            showSteeringSheet: .constant(false)
        )
    }
}
