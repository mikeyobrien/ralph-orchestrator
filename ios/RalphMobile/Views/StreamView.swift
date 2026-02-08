import SwiftUI

/// Real-time event stream view showing all events as they occur
/// Matches StreamView from ralph-mobile-prototype.jsx
struct StreamView: View {
    @ObservedObject var viewModel: SessionViewModel
    @State private var selectedFilter: EventFilter = .all
    @State private var showVerbose = false
    @State private var autoScroll = true
    @State private var searchText = ""

    enum EventFilter: String, CaseIterable {
        case all = "All"
        case toolCalls = "Tools"
        case hatActivations = "Hats"
        case events = "Events"
        case errors = "Errors"

        var icon: String {
            switch self {
            case .all: return "list.bullet"
            case .toolCalls: return "wrench"
            case .hatActivations: return "person.circle"
            case .events: return "bolt"
            case .errors: return "exclamationmark.triangle"
            }
        }
    }

    private var filteredEvents: [Event] {
        var events = viewModel.events

        // Apply filter
        switch selectedFilter {
        case .all:
            break
        case .toolCalls:
            events = events.filter { $0.type == "tool.call" }
        case .hatActivations:
            events = events.filter { $0.type == "hat.activated" }
        case .events:
            events = events.filter { $0.type == "event.published" }
        case .errors:
            events = events.filter { $0.status == "error" || $0.type.contains("error") }
        }

        // Apply search
        if !searchText.isEmpty {
            events = events.filter { event in
                event.type.localizedCaseInsensitiveContains(searchText) ||
                event.toolName?.localizedCaseInsensitiveContains(searchText) == true ||
                event.topic?.localizedCaseInsensitiveContains(searchText) == true
            }
        }

        return events
    }

    var body: some View {
        VStack(spacing: 0) {
            // Filter bar
            filterBar

            // Search bar
            searchBar

            // Event stream
            eventStream

            // Footer with controls
            footerControls
        }
        .background(CyberpunkTheme.bgPrimary)
        .navigationTitle("Event Stream")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                Menu {
                    Toggle("Verbose Mode", isOn: $showVerbose)
                    Toggle("Auto-scroll", isOn: $autoScroll)
                    Divider()
                    Button("Clear Events") {
                        viewModel.clearEvents()
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                        .foregroundColor(CyberpunkTheme.accentCyan)
                }
            }
        }
    }

    // MARK: - Filter Bar

    private var filterBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(EventFilter.allCases, id: \.self) { filter in
                    FilterChip(
                        title: filter.rawValue,
                        icon: filter.icon,
                        isSelected: selectedFilter == filter,
                        count: countForFilter(filter)
                    ) {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            selectedFilter = filter
                        }
                    }
                    .accessibilityIdentifier("stream-filter-\(filter.rawValue.lowercased())")
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
        }
        .background(CyberpunkTheme.bgSecondary)
    }

    private func countForFilter(_ filter: EventFilter) -> Int {
        switch filter {
        case .all:
            return viewModel.events.count
        case .toolCalls:
            return viewModel.events.filter { $0.type == "tool.call" }.count
        case .hatActivations:
            return viewModel.events.filter { $0.type == "hat.activated" }.count
        case .events:
            return viewModel.events.filter { $0.type == "event.published" }.count
        case .errors:
            return viewModel.events.filter { $0.status == "error" }.count
        }
    }

    // MARK: - Search Bar

    private var searchBar: some View {
        HStack(spacing: 12) {
            Image(systemName: "magnifyingglass")
                .foregroundColor(CyberpunkTheme.textMuted)

            TextField("Search events...", text: $searchText)
                .foregroundColor(CyberpunkTheme.textPrimary)
                .autocorrectionDisabled()
                .accessibilityIdentifier("stream-search-field")

            if !searchText.isEmpty {
                Button {
                    searchText = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
                .accessibilityIdentifier("stream-button-clear")
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(CyberpunkTheme.bgTertiary)
        .cornerRadius(8)
        .padding(.horizontal)
        .padding(.vertical, 8)
    }

    // MARK: - Event Stream

    private var eventStream: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 8) {
                    ForEach(filteredEvents) { event in
                        EventRow(event: event, showVerbose: showVerbose)
                            .id(event.id)
                    }
                }
                .padding()
            }
            .accessibilityIdentifier("stream-event-list")
            .onChange(of: filteredEvents.count) { _ in
                if autoScroll, let lastEvent = filteredEvents.last {
                    withAnimation {
                        proxy.scrollTo(lastEvent.id, anchor: .bottom)
                    }
                }
            }
        }
    }

    // MARK: - Footer Controls

    private var footerControls: some View {
        HStack {
            // Event count
            HStack(spacing: 4) {
                Circle()
                    .fill(viewModel.connectionState == .connected ? CyberpunkTheme.accentGreen : CyberpunkTheme.textMuted)
                    .frame(width: 6, height: 6)

                Text("\(filteredEvents.count) events")
                    .font(.caption.monospaced())
                    .foregroundColor(CyberpunkTheme.textSecondary)
            }

            Spacer()

            // Auto-scroll indicator
            if autoScroll {
                Label("Auto-scroll", systemImage: "arrow.down.circle")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.accentCyan)
            }
        }
        .padding(.horizontal)
        .padding(.vertical, 8)
        .background(CyberpunkTheme.bgSecondary)
    }
}

// MARK: - Filter Chip

private struct FilterChip: View {
    let title: String
    let icon: String
    let isSelected: Bool
    let count: Int
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.caption)

                Text(title)
                    .font(.caption)

                if count > 0 {
                    Text("\(count)")
                        .font(.caption2.bold())
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(
                            isSelected
                                ? CyberpunkTheme.bgPrimary.opacity(0.5)
                                : CyberpunkTheme.bgTertiary
                        )
                        .cornerRadius(8)
                }
            }
            .foregroundColor(isSelected ? CyberpunkTheme.bgPrimary : CyberpunkTheme.textSecondary)
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(isSelected ? CyberpunkTheme.accentCyan : CyberpunkTheme.bgCard)
            .cornerRadius(16)
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .stroke(
                        isSelected ? CyberpunkTheme.accentCyan : CyberpunkTheme.border,
                        lineWidth: 1
                    )
            )
        }
    }
}

// MARK: - Event Row

struct EventRow: View {
    let event: Event
    let showVerbose: Bool
    @State private var isExpanded = false

    private var eventColor: Color {
        switch event.type {
        case "tool.call": return CyberpunkTheme.accentMagenta
        case "hat.activated": return CyberpunkTheme.hatColor(for: event.hat ?? "")
        case "event.published": return CyberpunkTheme.accentYellow
        case "backpressure": return event.status == "pass" ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentRed
        default: return CyberpunkTheme.accentCyan
        }
    }

    private var eventIcon: String {
        switch event.type {
        case "tool.call": return "wrench"
        case "hat.activated": return "person.circle"
        case "event.published": return "bolt"
        case "backpressure": return "gauge"
        default: return "circle"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Main row
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack(spacing: 12) {
                    // Timestamp
                    Text(formatTime(event.timestamp))
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .frame(width: 50, alignment: .leading)

                    // Event type indicator
                    Circle()
                        .fill(eventColor)
                        .frame(width: 8, height: 8)
                        .shadow(color: eventColor.opacity(0.5), radius: 3)

                    // Event icon
                    Image(systemName: eventIcon)
                        .font(.caption)
                        .foregroundColor(eventColor)
                        .frame(width: 16)

                    // Event description
                    VStack(alignment: .leading, spacing: 2) {
                        Text(eventTitle)
                            .font(.subheadline)
                            .foregroundColor(CyberpunkTheme.textPrimary)
                            .lineLimit(1)

                        if let subtitle = eventSubtitle {
                            Text(subtitle)
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.textSecondary)
                                .lineLimit(1)
                        }
                    }

                    Spacer()

                    // Status badge
                    if let status = event.status {
                        StatusBadge(status: status)
                    }

                    // Expand chevron
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
                .padding(.vertical, 10)
                .padding(.horizontal, 12)
            }
            .buttonStyle(.plain)

            // Expanded verbose content
            if isExpanded && showVerbose {
                verboseContent
            }
        }
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(
                    isExpanded ? eventColor.opacity(0.5) : CyberpunkTheme.border,
                    lineWidth: 1
                )
        )
    }

    private var eventTitle: String {
        switch event.type {
        case "tool.call":
            return event.toolName ?? "tool call"
        case "hat.activated":
            return "Hat: \(event.hat ?? "unknown")"
        case "event.published":
            return "Event: \(event.topic ?? "unknown")"
        case "backpressure":
            return "Backpressure Check"
        default:
            return event.type
        }
    }

    private var eventSubtitle: String? {
        switch event.type {
        case "tool.call":
            if let input = event.input, let path = input["path"] as? String {
                return path
            } else if let input = event.input, let command = input["command"] as? String {
                return command.prefix(40) + (command.count > 40 ? "..." : "")
            }
            return nil
        case "hat.activated":
            return event.topic
        case "event.published":
            return "from \(event.hat ?? "unknown")"
        default:
            return nil
        }
    }

    private var verboseContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            Divider()
                .background(CyberpunkTheme.border)

            // Full JSON payload
            if let jsonString = formatEventJSON() {
                Text(jsonString)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .padding(8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(CyberpunkTheme.bgPrimary)
                    .cornerRadius(4)
            }

            // Duration if available
            if let duration = event.duration {
                HStack {
                    Image(systemName: "clock")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)

                    Text("Duration: \(duration)ms")
                        .font(.caption.monospaced())
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.bottom, 10)
    }

    private func formatEventJSON() -> String? {
        var dict: [String: Any] = [
            "type": event.type,
            "timestamp": ISO8601DateFormatter().string(from: event.timestamp)
        ]

        if let toolName = event.toolName { dict["tool"] = toolName }
        if let hat = event.hat { dict["hat"] = hat }
        if let topic = event.topic { dict["topic"] = topic }
        if let status = event.status { dict["status"] = status }
        if let input = event.input { dict["input"] = input }
        if let output = event.output { dict["output"] = output }

        if let data = try? JSONSerialization.data(withJSONObject: dict, options: [.prettyPrinted, .sortedKeys]),
           let string = String(data: data, encoding: .utf8) {
            return string
        }
        return nil
    }

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "HH:mm:ss"
        return formatter.string(from: date)
    }
}

// MARK: - Status Badge

private struct StatusBadge: View {
    let status: String

    private var color: Color {
        switch status.lowercased() {
        case "running": return CyberpunkTheme.accentCyan
        case "completed", "pass": return CyberpunkTheme.accentGreen
        case "pending": return CyberpunkTheme.accentYellow
        case "error", "fail", "failed": return CyberpunkTheme.accentRed
        default: return CyberpunkTheme.textMuted
        }
    }

    var body: some View {
        Text(status.lowercased())
            .font(.caption2.monospaced())
            .foregroundColor(color)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .cornerRadius(4)
            .overlay(
                RoundedRectangle(cornerRadius: 4)
                    .stroke(color.opacity(0.3), lineWidth: 1)
            )
    }
}

#Preview {
    NavigationStack {
        StreamView(
            viewModel: SessionViewModel(
                baseURL: URL(string: "http://localhost:8080")!,
                apiKey: ""
            )
        )
    }
}
