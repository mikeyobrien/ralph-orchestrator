import SwiftUI

/// Scrolling list of events with newest at top.
/// Styled with cyberpunk theme for consistency with V4 design.
struct EventFeedView: View {
    let events: [Event]

    var body: some View {
        List(events) { event in
            EventRowView(event: event)
                .listRowBackground(CyberpunkTheme.bgSecondary)
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .background(CyberpunkTheme.bgPrimary)
    }
}

#Preview {
    EventFeedView(events: [
        Event(
            timestamp: Date(),
            topic: "design.drafted",
            payload: "API contract and mobile UI components",
            iteration: 1,
            hat: "📐 Architect",
            triggered: nil
        ),
        Event(
            timestamp: Date().addingTimeInterval(-60),
            topic: "validation.passed",
            payload: "All 38 tests passing",
            iteration: 2,
            hat: "✅ Validator",
            triggered: nil
        ),
        Event(
            timestamp: Date().addingTimeInterval(-120),
            topic: "build.blocked",
            payload: "Missing dependency: notify crate not in Cargo.toml",
            iteration: 3,
            hat: "🔨 Builder",
            triggered: nil
        ),
        Event(
            timestamp: Date().addingTimeInterval(-180),
            topic: "task.started",
            payload: "Implementing SSE endpoint",
            iteration: 4,
            hat: "🔨 Builder",
            triggered: nil
        )
    ])
}
