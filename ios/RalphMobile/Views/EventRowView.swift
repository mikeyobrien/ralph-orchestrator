import SwiftUI

/// Displays a single event with timestamp, topic, and payload.
/// Styled with cyberpunk theme for consistency with V4 design.
struct EventRowView: View {
    let event: Event

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(formatTime(event.timestamp))
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .accessibilityIdentifier("event-row-timestamp")

                Text(event.topic ?? event.type)
                    .font(.system(.headline, design: .monospaced))
                    .foregroundColor(topicColor)
                    .accessibilityIdentifier("event-row-topic")
            }

            if let payload = event.payload, !payload.isEmpty {
                Text(payload)
                    .font(.system(.subheadline, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .lineLimit(2)
                    .accessibilityIdentifier("event-row-payload")
            }
        }
        .padding(.vertical, 4)
        .accessibilityIdentifier("event-row")
    }

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "HH:mm:ss"
        return formatter.string(from: date)
    }

    /// Color-code topics for visual distinction with cyberpunk colors.
    private var topicColor: Color {
        let topicString = event.topic ?? event.type
        if topicString.contains("error") || topicString.contains("blocked") {
            return CyberpunkTheme.accentRed
        } else if topicString.contains("complete") || topicString.contains("passed") {
            return CyberpunkTheme.accentGreen
        } else if topicString.contains("started") || topicString.contains("ready") {
            return CyberpunkTheme.accentCyan
        }
        return CyberpunkTheme.textPrimary
    }
}

#Preview {
    ZStack {
        CyberpunkTheme.bgPrimary.ignoresSafeArea()

        List {
            EventRowView(event: Event(
                timestamp: Date(),
                topic: "design.drafted",
                payload: "API contract and mobile UI components",
                iteration: 1,
                hat: "📐 Architect",
                triggered: nil
            ))

            EventRowView(event: Event(
                timestamp: Date().addingTimeInterval(-60),
                topic: "validation.passed",
                payload: "All 38 tests passing",
                iteration: 2,
                hat: "✅ Validator",
                triggered: nil
            ))

            EventRowView(event: Event(
                timestamp: Date().addingTimeInterval(-120),
                topic: "build.blocked",
                payload: "Missing dependency: notify crate not in Cargo.toml",
                iteration: 3,
                hat: "🔨 Builder",
                triggered: nil
            ))

            EventRowView(event: Event(
                timestamp: Date().addingTimeInterval(-180),
                topic: "task.started",
                payload: "Implementing SSE endpoint",
                iteration: 4,
                hat: "🔨 Builder",
                triggered: nil
            ))
        }
        .scrollContentBackground(.hidden)
        .listStyle(.plain)
    }
}
