import SwiftUI

/// The Unified Ralph View - Everything about a Ralph session in ONE scrollable view
/// Based on V3 architecture: "A Ralph IS a session"
/// All 10 sections from the spec are inline with collapsible sections where appropriate
struct UnifiedRalphView: View {
    @ObservedObject var viewModel: SessionViewModel
    @Binding var showSteeringSheet: Bool

    // Collapsible section states
    @State private var isEventStreamExpanded = true
    @State private var isScratchpadExpanded = true
    @State private var isHatFlowExpanded = true
    @State private var isIterationHistoryExpanded = false

    // Iteration history data
    @State private var iterations: [IterationItem] = []
    @State private var isLoadingIterations = false

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            ScrollView {
                VStack(spacing: 16) {
                    // Section 1: Header (always visible at top of scroll)
                    ralphHeaderSection

                    // Session completed/stopped banner
                    if !viewModel.isSessionActive, viewModel.currentSession != nil {
                        sessionEndedBanner
                    }

                    // Section 2: Metrics Grid
                    metricsGridSection

                    // Section 3: Context Window
                    contextWindowSection

                    // Section 4: Completion Promise
                    completionPromiseSection

                    // Section 5: Backpressure Status
                    backpressureSection

                    // Section 6: Hat Flow (Expandable)
                    hatFlowSection

                    // Section 6.5: Iteration History (Collapsible)
                    IterationHistorySection(
                        iterations: iterations,
                        isLoading: isLoadingIterations,
                        isExpanded: $isIterationHistoryExpanded
                    )
                    .task {
                        await loadIterations()
                    }

                    // Section 7: Signal Emission (only for active sessions)
                    if viewModel.isSessionActive {
                        signalEmissionSection
                    }

                    // Section 8: Event Stream (Collapsible)
                    eventStreamSection

                    // Section 9: Scratchpad (Collapsible)
                    scratchpadSection

                    // Section 10: Config Info Footer
                    configInfoFooter
                }
                .padding()
            }
            .background(CyberpunkTheme.bgPrimary)
            .refreshable {
                await viewModel.refreshSession()
            }

            // User Steering FAB (only for active sessions)
            if viewModel.isSessionActive {
                UserSteeringFAB(isPresented: $showSteeringSheet)
                    .padding(.trailing, 16)
                    .padding(.bottom, 16)
            }
        }
    }

    // MARK: - Section 1: Ralph Header

    private var ralphHeaderSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 12) {
                // Status dot with pulse animation for running
                StatusIndicator(status: currentStatus)

                VStack(alignment: .leading, spacing: 2) {
                    Text(sessionName)
                        .font(.title2.bold())
                        .foregroundColor(CyberpunkTheme.textPrimary)
                        .lineLimit(1)
                        .truncationMode(.middle)

                    Text(sessionSubtitle)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textSecondary)
                        .lineLimit(1)
                }

                Spacer()

                // Control buttons based on state
                controlButtons
            }

            // Connection indicator
            HStack(spacing: 6) {
                Circle()
                    .fill(connectionColor)
                    .frame(width: 6, height: 6)

                Text(connectionText)
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)
            }
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(statusColor.opacity(0.5), lineWidth: 1)
        )
        .accessibilityIdentifier("unified-ralph-status-header")
    }

    private var sessionName: String {
        viewModel.currentSession?.id ?? "No Session"
    }

    private var sessionSubtitle: String {
        guard let session = viewModel.currentSession else { return "No session" }
        let mode = session.mode ?? "unknown"
        if let elapsed = session.elapsedSeconds, elapsed > 0 {
            let hours = elapsed / 3600
            let minutes = (elapsed % 3600) / 60
            if hours > 0 {
                return "Mode: \(mode) • \(hours)h \(minutes)m"
            } else if minutes > 0 {
                return "Mode: \(mode) • \(minutes)m"
            } else {
                return "Mode: \(mode) • \(elapsed)s"
            }
        }
        return "Mode: \(mode)"
    }

    private var currentStatus: StatusIndicator.ExecutionStatus {
        switch viewModel.currentSession?.status {
        case "running": return .running
        case "paused": return .paused
        case "completed", "done": return .completed
        case "error": return .error
        default: return .idle
        }
    }

    private var statusColor: Color {
        currentStatus.color
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
        case .error: return "Connection Error"
        }
    }

    @ViewBuilder
    private var controlButtons: some View {
        switch currentStatus {
        case .running:
            HStack(spacing: 8) {
                Button {
                    Task { await viewModel.pauseRun() }
                } label: {
                    Image(systemName: "pause.fill")
                        .foregroundColor(CyberpunkTheme.bgPrimary)
                        .padding(8)
                        .background(CyberpunkTheme.accentYellow)
                        .clipShape(Circle())
                }
                .accessibilityIdentifier("unified-ralph-button-pause")

                Button {
                    Task { await viewModel.stopRun() }
                } label: {
                    Image(systemName: "stop.fill")
                        .foregroundColor(CyberpunkTheme.accentRed)
                        .padding(8)
                        .background(CyberpunkTheme.bgTertiary)
                        .clipShape(Circle())
                }
                .accessibilityIdentifier("unified-ralph-button-stop")
            }

        case .paused:
            HStack(spacing: 8) {
                Button {
                    Task { await viewModel.resumeRun() }
                } label: {
                    Image(systemName: "play.fill")
                        .foregroundColor(CyberpunkTheme.bgPrimary)
                        .padding(8)
                        .background(CyberpunkTheme.accentGreen)
                        .clipShape(Circle())
                }
                .accessibilityIdentifier("unified-ralph-button-resume")

                Button {
                    Task { await viewModel.stopRun() }
                } label: {
                    Image(systemName: "stop.fill")
                        .foregroundColor(CyberpunkTheme.accentRed)
                        .padding(8)
                        .background(CyberpunkTheme.bgTertiary)
                        .clipShape(Circle())
                }
                .accessibilityIdentifier("unified-ralph-button-stop")
            }

        case .completed:
            EmptyView()

        default:
            Button {
                Task { await viewModel.startRun() }
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "play.fill")
                    Text("Start")
                        .font(.caption.bold())
                }
                .foregroundColor(CyberpunkTheme.bgPrimary)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(CyberpunkTheme.accentGreen)
                .cornerRadius(8)
            }
            .accessibilityIdentifier("unified-ralph-button-start")
        }
    }

    // MARK: - Section 2: Metrics Grid

    private var metricsGridSection: some View {
        LazyVGrid(columns: [
            GridItem(.flexible()),
            GridItem(.flexible()),
            GridItem(.flexible())
        ], spacing: 12) {
            MetricCard(
                label: "Iteration",
                value: "\(viewModel.currentSession?.iteration ?? 0)/\(viewModel.currentSession?.total ?? 100)",
                icon: "repeat",
                color: CyberpunkTheme.accentCyan
            )

            MetricCard(
                label: "Runtime",
                value: formatRuntime(viewModel.currentSession?.elapsedSeconds ?? 0),
                icon: "clock",
                color: CyberpunkTheme.accentYellow
            )

            MetricCard(
                label: "Cost",
                value: String(format: "$%.2f", viewModel.tokenMetrics.estimatedCost),
                icon: "dollarsign.circle",
                color: CyberpunkTheme.accentGreen
            )
        }
    }

    private func formatRuntime(_ seconds: Int) -> String {
        let hours = seconds / 3600
        let minutes = (seconds % 3600) / 60
        let secs = seconds % 60

        if hours > 0 {
            return String(format: "%02d:%02d:%02d", hours, minutes, secs)
        }
        return String(format: "%02d:%02d", minutes, secs)
    }

    // MARK: - Section 3: Context Window

    private var contextWindowSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("CONTEXT WINDOW")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .kerning(1)

                Spacer()

                Text("\(formatTokens(viewModel.tokenMetrics.inputTokens + viewModel.tokenMetrics.outputTokens))/\(formatTokens(viewModel.tokenMetrics.maxTokens))")
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(contextColor)
            }

            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    // Background track
                    RoundedRectangle(cornerRadius: 4)
                        .fill(CyberpunkTheme.bgTertiary)
                        .frame(height: 8)

                    // Progress fill
                    RoundedRectangle(cornerRadius: 4)
                        .fill(contextColor)
                        .frame(width: geometry.size.width * contextProgress, height: 8)
                        .shadow(color: contextColor.opacity(0.5), radius: 4)
                }
            }
            .frame(height: 8)

            // Token breakdown
            HStack {
                Label("\(formatTokens(viewModel.tokenMetrics.inputTokens)) input", systemImage: "arrow.right.circle")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textSecondary)

                Spacer()

                Label("\(formatTokens(viewModel.tokenMetrics.outputTokens)) output", systemImage: "arrow.left.circle")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textSecondary)
            }
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
    }

    private var contextProgress: Double {
        let total = viewModel.tokenMetrics.inputTokens + viewModel.tokenMetrics.outputTokens
        let max = viewModel.tokenMetrics.maxTokens
        guard max > 0 else { return 0 }
        return min(Double(total) / Double(max), 1.0)
    }

    private var contextColor: Color {
        let percentage = contextProgress * 100
        if percentage > 80 {
            return CyberpunkTheme.accentRed
        } else if percentage > 60 {
            return CyberpunkTheme.accentYellow
        }
        return CyberpunkTheme.accentPurple
    }

    private func formatTokens(_ count: Int) -> String {
        if count >= 1000 {
            return String(format: "%.1fK", Double(count) / 1000)
        }
        return "\(count)"
    }

    // MARK: - Section 4: Completion Promise

    /// The completion event that signals the Ralph session is done.
    /// Extracted from config or defaults to standard completion event.
    private var completionPromiseText: String {
        // TODO: Parse from config content when API provides structured data
        // For now, use the canonical event name from Ralph orchestrator
        "loop.complete"
    }

    private var completionPromiseSection: some View {
        HStack(spacing: 8) {
            Image(systemName: "flag.checkered")
                .foregroundColor(CyberpunkTheme.accentGreen)

            Text("Completion Promise:")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textSecondary)

            Text(completionPromiseText)
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.accentGreen)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(CyberpunkTheme.accentGreen.opacity(0.15))
                .cornerRadius(4)

            Spacer()
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
    }

    // MARK: - Section 5: Backpressure Status

    private var backpressureSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("BACKPRESSURE")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .kerning(1)

            BackpressureStatusView(checks: [
                .init(name: "tests", passed: viewModel.backpressure?.testsPass ?? false),
                .init(name: "lint", passed: viewModel.backpressure?.lintPass ?? false),
                .init(name: "typecheck", passed: viewModel.backpressure?.typecheckPass ?? false)
            ])
        }
    }

    // MARK: - Section 6: Hat Flow (Expandable)

    private var hatFlowSection: some View {
        CollapsibleSection(
            title: "HAT FLOW",
            isExpanded: $isHatFlowExpanded,
            accentColor: CyberpunkTheme.accentCyan,
            badge: viewModel.currentSession?.hat
        ) {
            HatFlowView(
                currentHat: viewModel.currentSession?.hat ?? "idle",
                iteration: viewModel.currentSession?.iteration ?? 0,
                triggerEvent: viewModel.currentSession?.triggerEvent,
                publishedEvents: viewModel.currentSession?.availablePublishes ?? []
            )
        }
        .accessibilityIdentifier("unified-ralph-section-hat-flow")
    }

    // MARK: - Section 7: Signal Emission

    private var signalEmissionSection: some View {
        SignalEmitterView(
            acceptedSignals: ["user.guidance", "user.pause", "user.priority", "user.abort"],
            onSendSignal: { signalType, message in
                Task {
                    await viewModel.emitSignal(type: signalType, message: message)
                }
            }
        )
    }

    // MARK: - Section 8: Event Stream (Collapsible)

    private var eventStreamSection: some View {
        CollapsibleSection(
            title: "EVENT STREAM",
            isExpanded: $isEventStreamExpanded,
            accentColor: CyberpunkTheme.accentMagenta,
            badge: viewModel.isSessionActive ? "\(viewModel.events.count)" : "ENDED"
        ) {
            if !viewModel.isSessionActive && viewModel.events.isEmpty {
                // Completed session with no live events
                VStack(spacing: 12) {
                    Image(systemName: "clock.badge.checkmark")
                        .font(.system(size: 32))
                        .foregroundColor(CyberpunkTheme.textMuted)
                    Text("Session completed — no live events")
                        .font(.system(.body, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 24)
            } else {
                VerboseEventStreamView(
                    events: viewModel.events,
                    currentHat: viewModel.currentSession?.hat,
                    sessionStartTime: sessionStartTime,
                    onClearEvents: {
                        viewModel.clearEvents()
                    }
                )
            }
        }
        .accessibilityIdentifier("unified-ralph-section-event-stream")
    }

    /// Compute session start time from the first event or session data
    private var sessionStartTime: Date? {
        // Try to get from session startTime (parsed Date), or fall back to first event
        if let startTime = viewModel.currentSession?.startTime {
            return startTime
        }
        return viewModel.events.first?.timestamp
    }

    // MARK: - Iteration History Loading

    private func loadIterations() async {
        guard let sessionId = viewModel.currentSession?.id else { return }
        isLoadingIterations = true
        do {
            let response = try await RalphAPIClient.shared.getIterations(sessionId: sessionId)
            iterations = response.iterations
        } catch {
            #if DEBUG
            print("Failed to load iterations: \(error)")
            #endif
        }
        isLoadingIterations = false
    }

    // MARK: - Section 9: Scratchpad (Collapsible)

    private var scratchpadSection: some View {
        CollapsibleSection(
            title: "SCRATCHPAD",
            isExpanded: $isScratchpadExpanded,
            accentColor: CyberpunkTheme.accentYellow,
            badge: viewModel.scratchpadContent != nil ? (viewModel.isSessionActive ? "LIVE" : "HISTORICAL") : nil,
            badgeColor: CyberpunkTheme.accentGreen
        ) {
            ScratchpadContentView(
                content: viewModel.scratchpadContent,
                onRefresh: {
                    Task { await viewModel.fetchScratchpad() }
                }
            )
        }
        .accessibilityIdentifier("unified-ralph-section-scratchpad")
    }

    // MARK: - Session Ended Banner

    private var sessionEndedBanner: some View {
        let status = viewModel.currentSession?.status ?? "completed"
        let isCompleted = status == "completed" || status == "done"
        let icon = isCompleted ? "checkmark.circle.fill" : "stop.circle.fill"
        let text = isCompleted ? "Session Completed" : "Session Stopped"
        let color = isCompleted ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentYellow

        return HStack(spacing: 10) {
            Image(systemName: icon)
                .foregroundColor(color)
            Text(text.uppercased())
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(color)
                .kerning(1)
            Spacer()
            Text("READ-ONLY")
                .font(.system(.caption2, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(CyberpunkTheme.bgTertiary)
                .cornerRadius(4)
        }
        .padding()
        .background(color.opacity(0.1))
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(color.opacity(0.3), lineWidth: 1)
        )
    }

    // MARK: - Section 10: Config Info Footer

    private var configInfoFooter: some View {
        HStack(spacing: 16) {
            HStack(spacing: 4) {
                Text("Session:")
                    .foregroundColor(CyberpunkTheme.textMuted)
                Text(viewModel.currentSession?.id.prefix(8) ?? "N/A")
                    .foregroundColor(CyberpunkTheme.accentCyan)
            }

            Text("•")
                .foregroundColor(CyberpunkTheme.textMuted)

            HStack(spacing: 4) {
                Text("Iteration:")
                    .foregroundColor(CyberpunkTheme.textMuted)
                Text("\(viewModel.currentSession?.iteration ?? 0)")
                    .foregroundColor(CyberpunkTheme.accentPurple)
            }

            Spacer()

            Text(viewModel.currentSession?.mode ?? "N/A")
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .font(.system(.caption, design: .monospaced))
        .padding()
        .background(CyberpunkTheme.bgSecondary)
        .cornerRadius(8)
        .accessibilityIdentifier("unified-ralph-section-config-info")
    }
}

// MARK: - Supporting Views

/// Metric card for the metrics grid
private struct MetricCard: View {
    let label: String
    let value: String
    let icon: String
    let color: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Image(systemName: icon)
                    .font(.caption)
                    .foregroundColor(color)
                Spacer()
            }

            Text(value)
                .font(.system(.title3, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textPrimary)

            Text(label)
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textSecondary)
        }
        .padding(12)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(color.opacity(0.3), lineWidth: 1)
        )
    }
}

/// Collapsible section container
struct CollapsibleSection<Content: View>: View {
    let title: String
    @Binding var isExpanded: Bool
    let accentColor: Color
    var badge: String? = nil
    var badgeColor: Color? = nil
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            Button {
                withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(.caption.bold())
                        .foregroundColor(accentColor)

                    Text(title)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .kerning(1)

                    if let badge = badge {
                        Text(badge)
                            .font(.caption2.bold())
                            .foregroundColor(badgeColor ?? accentColor)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background((badgeColor ?? accentColor).opacity(0.2))
                            .cornerRadius(4)
                    }

                    Spacer()
                }
                .padding(.vertical, 8)
            }
            .buttonStyle(.plain)

            // Content
            if isExpanded {
                content()
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(isExpanded ? accentColor.opacity(0.3) : CyberpunkTheme.border, lineWidth: 1)
        )
    }
}

#Preview {
    UnifiedRalphView(
        viewModel: SessionViewModel(
            baseURL: URL(string: "http://localhost:8080")!,
            apiKey: ""
        ),
        showSteeringSheet: .constant(false)
    )
}
