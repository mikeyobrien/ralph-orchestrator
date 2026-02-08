import SwiftUI

/// Live insights header for the event stream showing real-time analytics.
/// Displays: events/min rate, current hat, success rate, elapsed time.
struct EventStreamInsightsView: View {
    let events: [Event]
    let currentHat: String?
    let sessionStartTime: Date?

    @State private var currentTime = Date()

    // Update elapsed time every second
    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 8) {
            // Header
            HStack {
                Image(systemName: "chart.line.uptrend.xyaxis")
                    .foregroundColor(CyberpunkTheme.accentCyan)

                Text("LIVE INSIGHTS")
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)
                    .kerning(1)

                Spacer()

                // Live indicator
                HStack(spacing: 4) {
                    Circle()
                        .fill(CyberpunkTheme.accentGreen)
                        .frame(width: 6, height: 6)
                        .modifier(PulseAnimation())

                    Text("LIVE")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.accentGreen)
                }
            }

            // Metrics Grid
            HStack(spacing: 12) {
                InsightMetric(
                    icon: "bolt.fill",
                    value: "\(eventsPerMinute)",
                    label: "events/min",
                    color: eventsPerMinute > 10 ? CyberpunkTheme.accentYellow : CyberpunkTheme.accentCyan
                )

                InsightMetric(
                    icon: hatIcon,
                    value: currentHat ?? "idle",
                    label: "active hat",
                    color: hatColor
                )

                InsightMetric(
                    icon: successRate >= 90 ? "checkmark.circle.fill" : "exclamationmark.triangle.fill",
                    value: "\(Int(successRate))%",
                    label: "success",
                    color: successRate >= 90 ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentYellow
                )

                InsightMetric(
                    icon: "clock.fill",
                    value: formattedElapsed,
                    label: "elapsed",
                    color: CyberpunkTheme.accentPurple
                )
            }
        }
        .padding(12)
        .background(CyberpunkTheme.bgTertiary)
        .cornerRadius(10)
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(CyberpunkTheme.accentCyan.opacity(0.3), lineWidth: 1)
        )
        .onReceive(timer) { _ in
            currentTime = Date()
        }
    }

    // MARK: - Computed Properties

    private var eventsPerMinute: Int {
        guard !events.isEmpty else { return 0 }

        let oneMinuteAgo = Date().addingTimeInterval(-60)
        let recentEvents = events.filter { $0.timestamp > oneMinuteAgo }
        return recentEvents.count
    }

    private var successRate: Double {
        guard !events.isEmpty else { return 100 }

        let errorCount = events.filter { event in
            event.status == "error" ||
            (event.topic?.contains("error") ?? false) ||
            (event.topic?.contains("blocked") ?? false) ||
            (event.topic?.contains("failed") ?? false)
        }.count

        let successCount = events.count - errorCount
        return Double(successCount) / Double(events.count) * 100
    }

    private var formattedElapsed: String {
        guard let start = sessionStartTime else { return "0:00" }

        let elapsed = Int(currentTime.timeIntervalSince(start))
        let hours = elapsed / 3600
        let minutes = (elapsed % 3600) / 60
        let seconds = elapsed % 60

        if hours > 0 {
            return String(format: "%d:%02d:%02d", hours, minutes, seconds)
        }
        return String(format: "%d:%02d", minutes, seconds)
    }

    private var hatIcon: String {
        guard let hat = currentHat?.lowercased() else { return "person.circle" }

        switch hat {
        case "planner", "planning": return "map"
        case "builder", "building": return "hammer"
        case "fixer", "fixing": return "wrench"
        case "reviewer", "reviewing": return "eye"
        case "architect": return "building.2"
        case "validator": return "checkmark.shield"
        case "loop": return "repeat"
        default: return "person.circle"
        }
    }

    private var hatColor: Color {
        guard let hat = currentHat?.lowercased() else { return CyberpunkTheme.textMuted }

        switch hat {
        case "planner", "planning": return CyberpunkTheme.accentCyan
        case "builder", "building": return CyberpunkTheme.accentYellow
        case "fixer", "fixing": return CyberpunkTheme.accentRed
        case "reviewer", "reviewing": return CyberpunkTheme.accentPurple
        case "architect": return CyberpunkTheme.accentMagenta
        case "validator": return CyberpunkTheme.accentGreen
        case "loop": return CyberpunkTheme.textSecondary
        default: return CyberpunkTheme.textMuted
        }
    }
}

// MARK: - Insight Metric Component

private struct InsightMetric: View {
    let icon: String
    let value: String
    let label: String
    let color: Color

    var body: some View {
        VStack(spacing: 4) {
            Image(systemName: icon)
                .font(.system(size: 14))
                .foregroundColor(color)

            Text(value)
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textPrimary)
                .lineLimit(1)
                .minimumScaleFactor(0.7)

            Text(label)
                .font(.system(.caption2, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .background(CyberpunkTheme.bgSecondary)
        .cornerRadius(8)
    }
}

// MARK: - Pulse Animation

private struct PulseAnimation: ViewModifier {
    @State private var isPulsing = false

    func body(content: Content) -> some View {
        content
            .scaleEffect(isPulsing ? 1.3 : 1.0)
            .opacity(isPulsing ? 0.7 : 1.0)
            .animation(
                .easeInOut(duration: 1.0).repeatForever(autoreverses: true),
                value: isPulsing
            )
            .onAppear { isPulsing = true }
    }
}

// MARK: - Preview

#Preview {
    VStack(spacing: 16) {
        EventStreamInsightsView(
            events: [
                Event(timestamp: Date(), topic: "task.start", payload: "Starting"),
                Event(timestamp: Date().addingTimeInterval(-10), topic: "build.passed", payload: "OK"),
                Event(timestamp: Date().addingTimeInterval(-20), topic: "test.passed", payload: "OK"),
                Event(timestamp: Date().addingTimeInterval(-30), topic: "lint.passed", payload: "OK"),
                Event(timestamp: Date().addingTimeInterval(-40), topic: "build.blocked", payload: "Error"),
            ],
            currentHat: "builder",
            sessionStartTime: Date().addingTimeInterval(-754)
        )

        EventStreamInsightsView(
            events: [],
            currentHat: nil,
            sessionStartTime: nil
        )
    }
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}
