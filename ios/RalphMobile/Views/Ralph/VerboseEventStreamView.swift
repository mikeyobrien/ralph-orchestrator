import SwiftUI

/// Event stream with verbose mode toggle and live insights.
/// Redesigned for maximum readability with type-specific event rendering.
struct VerboseEventStreamView: View {
    let events: [Event]
    let currentHat: String?
    let sessionStartTime: Date?
    let onClearEvents: () -> Void

    @State private var isVerboseMode: Bool = true  // Default to verbose for better UX
    @State private var filterCategory: EventCategoryFilter = .all
    @State private var searchText: String = ""
    @State private var showInsights: Bool = true

    private var filteredEvents: [Event] {
        var result = events

        // Apply category filter
        switch filterCategory {
        case .all:
            break
        case .hats:
            result = result.filter { $0.eventCategory == .hat }
        case .gates:
            result = result.filter { $0.eventCategory == .gate }
        case .tasks:
            result = result.filter { $0.eventCategory == .task }
        case .tools:
            result = result.filter { $0.eventCategory == .tool }
        case .errors:
            result = result.filter { $0.isError }
        }

        // Apply search
        if !searchText.isEmpty {
            let query = searchText.lowercased()
            result = result.filter { event in
                (event.topic?.lowercased().contains(query) ?? false) ||
                (event.payload?.lowercased().contains(query) ?? false) ||
                (event.toolName?.lowercased().contains(query) ?? false) ||
                (event.output?.lowercased().contains(query) ?? false) ||
                (event.hat?.lowercased().contains(query) ?? false)
            }
        }

        return result
    }

    // Sort events newest first for live monitoring
    private var sortedEvents: [Event] {
        filteredEvents.sorted { $0.timestamp > $1.timestamp }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Insights header (collapsible)
            if showInsights && !events.isEmpty {
                EventStreamInsightsView(
                    events: events,
                    currentHat: currentHat,
                    sessionStartTime: sessionStartTime
                )
                .padding(.horizontal, 12)
                .padding(.top, 12)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }

            // Controls header
            streamHeader

            Divider()
                .background(CyberpunkTheme.border)

            // Event list
            if sortedEvents.isEmpty {
                emptyStateView
            } else {
                eventListView
            }
        }
        .background(CyberpunkTheme.bgSecondary)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
    }

    // MARK: - Header

    private var streamHeader: some View {
        VStack(spacing: 8) {
            HStack {
                // Title with count
                HStack(spacing: 6) {
                    Image(systemName: "list.bullet.rectangle")
                        .foregroundColor(CyberpunkTheme.accentCyan)

                    Text("EVENT STREAM")
                        .font(.system(.caption, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    Text("[\(sortedEvents.count)]")
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                }

                Spacer()

                // Insights toggle
                Button {
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                        showInsights.toggle()
                    }
                } label: {
                    Image(systemName: showInsights ? "chart.bar.fill" : "chart.bar")
                        .font(.caption)
                        .foregroundColor(showInsights ? CyberpunkTheme.accentCyan : CyberpunkTheme.textMuted)
                }
                .accessibilityIdentifier("event-stream-toggle-insights")

                // Verbose mode toggle
                Button {
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                        isVerboseMode.toggle()
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: isVerboseMode ? "eye.fill" : "eye")
                        Text(isVerboseMode ? "CARDS" : "LIST")
                    }
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(isVerboseMode ? CyberpunkTheme.accentCyan : CyberpunkTheme.textMuted)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(isVerboseMode ? CyberpunkTheme.accentCyan.opacity(0.15) : Color.clear)
                    .cornerRadius(4)
                }
                .accessibilityIdentifier("event-stream-toggle-verbose")

                // Clear button
                Button {
                    onClearEvents()
                } label: {
                    Image(systemName: "trash")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
                .accessibilityIdentifier("event-stream-clear-button")
            }

            // Filter pills and search
            HStack(spacing: 6) {
                // Filter pills - scrollable for all categories
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(EventCategoryFilter.allCases, id: \.self) { filter in
                            filterPill(filter)
                        }
                    }
                }

                // Search field
                HStack(spacing: 4) {
                    Image(systemName: "magnifyingglass")
                        .font(.caption2)
                        .foregroundColor(CyberpunkTheme.textMuted)

                    TextField("Search", text: $searchText)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textPrimary)
                        .textFieldStyle(.plain)
                        .frame(width: 80)
                        .accessibilityIdentifier("event-stream-search-field")

                    if !searchText.isEmpty {
                        Button {
                            searchText = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                                .font(.caption2)
                                .foregroundColor(CyberpunkTheme.textMuted)
                        }
                    }
                }
                .padding(.horizontal, 6)
                .padding(.vertical, 4)
                .background(CyberpunkTheme.bgPrimary)
                .cornerRadius(4)
            }
        }
        .padding(12)
    }

    private func filterPill(_ filter: EventCategoryFilter) -> some View {
        let isSelected = filterCategory == filter
        let count = countForFilter(filter)

        return Button {
            withAnimation(.spring(response: 0.2, dampingFraction: 0.8)) {
                filterCategory = filter
            }
        } label: {
            HStack(spacing: 4) {
                Image(systemName: filter.icon)
                    .font(.system(size: 10))
                Text(filter.rawValue)
                    .font(.system(.caption2, design: .monospaced))
                if count > 0 && filter != .all {
                    Text("\(count)")
                        .font(.system(.caption2, design: .monospaced).bold())
                        .padding(.horizontal, 4)
                        .padding(.vertical, 1)
                        .background(isSelected ? CyberpunkTheme.bgPrimary.opacity(0.3) : CyberpunkTheme.bgTertiary)
                        .cornerRadius(3)
                }
            }
            .foregroundColor(isSelected ? CyberpunkTheme.bgPrimary : CyberpunkTheme.textMuted)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(isSelected ? filter.color : CyberpunkTheme.bgTertiary)
            .cornerRadius(6)
        }
        .accessibilityIdentifier("event-stream-filter-\(filter.rawValue.lowercased())")
    }

    private func countForFilter(_ filter: EventCategoryFilter) -> Int {
        switch filter {
        case .all: return events.count
        case .hats: return events.filter { $0.eventCategory == .hat }.count
        case .gates: return events.filter { $0.eventCategory == .gate }.count
        case .tasks: return events.filter { $0.eventCategory == .task }.count
        case .tools: return events.filter { $0.eventCategory == .tool }.count
        case .errors: return events.filter { $0.isError }.count
        }
    }

    // MARK: - Event List

    private var eventListView: some View {
        ScrollView {
            LazyVStack(spacing: isVerboseMode ? 10 : 4) {
                ForEach(sortedEvents) { event in
                    if isVerboseMode {
                        VerboseEventRowView(event: event)
                            .transition(.opacity.combined(with: .scale(scale: 0.95)))
                    } else {
                        compactEventRow(event)
                    }
                }
            }
            .padding(10)
        }
        .frame(maxHeight: 500)
    }

    private func compactEventRow(_ event: Event) -> some View {
        HStack(spacing: 8) {
            // Timestamp
            Text(formatTime(event.timestamp))
                .font(.system(.caption2, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .frame(width: 65, alignment: .leading)

            // Category indicator
            Image(systemName: event.eventCategory.icon)
                .font(.system(size: 10))
                .foregroundColor(categoryColor(event))
                .frame(width: 16)

            // Content
            if let toolName = event.toolName {
                Text(toolName)
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)

                if let status = event.status {
                    statusBadge(status)
                }
            } else if let hat = event.hat, event.eventCategory == .hat {
                Text(hat)
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentYellow)
            } else {
                Text(event.topic ?? event.type)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(event.isError ? CyberpunkTheme.accentRed :
                                     event.isSuccess ? CyberpunkTheme.accentGreen :
                                     CyberpunkTheme.textPrimary)
            }

            Spacer()

            // Duration for tools
            if let duration = event.duration {
                Text("\(duration)ms")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            // Iteration
            if let iteration = event.iteration {
                Text("#\(iteration)")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentPurple)
            }
        }
        .padding(.vertical, 4)
        .padding(.horizontal, 8)
        .background(event.isError ? CyberpunkTheme.accentRed.opacity(0.1) : Color.clear)
        .cornerRadius(4)
    }

    private func statusBadge(_ status: String) -> some View {
        let color = statusColor(status)
        return Text(status)
            .font(.system(.caption2, design: .monospaced))
            .foregroundColor(color)
            .padding(.horizontal, 4)
            .padding(.vertical, 1)
            .background(color.opacity(0.15))
            .cornerRadius(3)
    }

    // MARK: - Empty State

    private var emptyStateView: some View {
        VStack(spacing: 12) {
            Image(systemName: "antenna.radiowaves.left.and.right")
                .font(.largeTitle)
                .foregroundColor(CyberpunkTheme.textMuted)
                .modifier(PulseEffectModifier())

            VStack(spacing: 4) {
                Text(searchText.isEmpty ? "Waiting for events..." : "No matching events")
                    .font(.subheadline)
                    .foregroundColor(CyberpunkTheme.textPrimary)

                Text(searchText.isEmpty ?
                     "Events will appear here when the session starts" :
                     "Try a different search term or filter")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: 150)
    }

    // MARK: - Helpers

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "HH:mm:ss"
        return formatter.string(from: date)
    }

    private func categoryColor(_ event: Event) -> Color {
        if event.isError { return CyberpunkTheme.accentRed }
        if event.isSuccess { return CyberpunkTheme.accentGreen }

        switch event.eventCategory {
        case .hat: return CyberpunkTheme.accentYellow
        case .gate: return event.isSuccess ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentRed
        case .task: return CyberpunkTheme.accentCyan
        case .tool: return CyberpunkTheme.accentPurple
        case .debug: return CyberpunkTheme.textMuted
        }
    }

    private func statusColor(_ status: String) -> Color {
        switch status.lowercased() {
        case "completed": return CyberpunkTheme.accentGreen
        case "running": return CyberpunkTheme.accentYellow
        case "error": return CyberpunkTheme.accentRed
        case "pending": return CyberpunkTheme.textMuted
        default: return CyberpunkTheme.textMuted
        }
    }
}

// MARK: - Event Category Filter

enum EventCategoryFilter: String, CaseIterable {
    case all = "All"
    case hats = "Hats"
    case gates = "Gates"
    case tasks = "Tasks"
    case tools = "Tools"
    case errors = "Errors"

    var icon: String {
        switch self {
        case .all: return "list.bullet"
        case .hats: return "person.crop.circle"
        case .gates: return "door.left.hand.closed"
        case .tasks: return "target"
        case .tools: return "wrench.and.screwdriver"
        case .errors: return "exclamationmark.triangle"
        }
    }

    var color: Color {
        switch self {
        case .all: return CyberpunkTheme.accentCyan
        case .hats: return CyberpunkTheme.accentYellow
        case .gates: return CyberpunkTheme.accentGreen
        case .tasks: return CyberpunkTheme.accentCyan
        case .tools: return CyberpunkTheme.accentPurple
        case .errors: return CyberpunkTheme.accentRed
        }
    }
}

// MARK: - Pulse Effect Modifier (iOS 17+ compatible)

private struct PulseEffectModifier: ViewModifier {
    @State private var isAnimating = false

    func body(content: Content) -> some View {
        if #available(iOS 17.0, *) {
            content.symbolEffect(.pulse)
        } else {
            // Fallback for iOS 16: simple opacity animation
            content
                .opacity(isAnimating ? 0.5 : 1.0)
                .animation(.easeInOut(duration: 1.0).repeatForever(autoreverses: true), value: isAnimating)
                .onAppear { isAnimating = true }
        }
    }
}

// MARK: - Preview

#Preview("With Events") {
    VerboseEventStreamView(
        events: [
            Event(
                timestamp: Date(),
                topic: "hat.activated",
                iteration: 5,
                hat: "builder",
                triggered: "planner",
                type: "hat.activated"
            ),
            Event(
                timestamp: Date().addingTimeInterval(-10),
                type: "tool.call",
                toolName: "Read",
                status: "completed",
                input: ["file_path": "/src/main.swift"],
                output: "import Foundation...",
                duration: 45
            ),
            Event(
                timestamp: Date().addingTimeInterval(-20),
                topic: "build.passed",
                payload: "All 38 tests passing",
                type: "backpressure"
            ),
            Event(
                timestamp: Date().addingTimeInterval(-30),
                topic: "task.start",
                payload: "# Task: Build API\n\n## Objective\n\nCreate REST endpoints...",
                iteration: 1,
                type: "event.published"
            ),
            Event(
                timestamp: Date().addingTimeInterval(-40),
                topic: "typecheck.blocked",
                payload: "Cannot find type 'UserService' in scope",
                type: "backpressure"
            ),
            Event(
                timestamp: Date().addingTimeInterval(-50),
                type: "tool.call",
                toolName: "Glob",
                status: "completed",
                input: ["pattern": "**/*.swift"],
                output: "Found 42 files",
                duration: 120
            )
        ],
        currentHat: "builder",
        sessionStartTime: Date().addingTimeInterval(-754),
        onClearEvents: {
            #if DEBUG
            print("Clear events")
            #endif
        }
    )
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}

#Preview("Empty State") {
    VerboseEventStreamView(
        events: [],
        currentHat: nil,
        sessionStartTime: nil,
        onClearEvents: {}
    )
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}
