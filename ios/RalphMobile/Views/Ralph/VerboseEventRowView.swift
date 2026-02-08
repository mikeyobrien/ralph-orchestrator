import SwiftUI

/// Verbose event row with type-specific card designs.
/// Renders differently based on event category: hat, gate, task, tool, debug.
struct VerboseEventRowView: View {
    let event: Event
    @State private var isExpanded: Bool = false

    var body: some View {
        Group {
            switch event.eventCategory {
            case .hat:
                HatTransitionCard(event: event)
            case .gate:
                GateStatusCard(event: event, isExpanded: $isExpanded)
            case .task:
                TaskLifecycleCard(event: event, isExpanded: $isExpanded)
            case .tool:
                ToolCallRow(event: event, isExpanded: $isExpanded)
            case .debug:
                DebugEventRow(event: event)
            }
        }
        .accessibilityIdentifier("verbose-event-row-\(event.eventCategory.rawValue)")
    }
}

// MARK: - Hat Transition Card

/// Prominent card for hat changes - these are key moments in orchestration
private struct HatTransitionCard: View {
    let event: Event

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header
            HStack(spacing: 8) {
                // Hat icon
                Image(systemName: hatIcon)
                    .font(.title3)
                    .foregroundColor(hatColor)
                    .frame(width: 32, height: 32)
                    .background(hatColor.opacity(0.2))
                    .clipShape(Circle())

                VStack(alignment: .leading, spacing: 2) {
                    Text("HAT CHANGE")
                        .font(.system(.caption2, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .kerning(1)

                    Text(event.hat ?? "unknown")
                        .font(.system(.headline, design: .monospaced).bold())
                        .foregroundColor(hatColor)
                }

                Spacer()

                // Timestamp
                Text(formatTime(event.timestamp))
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            // Triggered info
            if let triggered = event.triggered {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.right.circle")
                        .font(.caption2)
                    Text("Triggered: \(triggered)")
                        .font(.system(.caption, design: .monospaced))
                }
                .foregroundColor(CyberpunkTheme.textSecondary)
            }

            // Iteration badge
            if let iteration = event.iteration {
                HStack(spacing: 4) {
                    Image(systemName: "repeat")
                        .font(.caption2)
                    Text("Iteration \(iteration)")
                        .font(.system(.caption, design: .monospaced))
                }
                .foregroundColor(CyberpunkTheme.accentCyan)
            }
        }
        .padding(12)
        .background(hatColor.opacity(0.1))
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(hatColor.opacity(0.4), lineWidth: 2)
        )
    }

    private var hatIcon: String {
        guard let hat = event.hat?.lowercased() else { return "person.circle" }
        switch hat {
        case "planner", "planning": return "map.fill"
        case "builder", "building": return "hammer.fill"
        case "fixer", "fixing": return "wrench.fill"
        case "reviewer", "reviewing": return "eye.fill"
        case "architect": return "building.2.fill"
        case "validator": return "checkmark.shield.fill"
        case "loop": return "repeat"
        default: return "person.circle.fill"
        }
    }

    private var hatColor: Color {
        guard let hat = event.hat?.lowercased() else { return CyberpunkTheme.textMuted }
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

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "h:mm:ss a"
        return formatter.string(from: date)
    }
}

// MARK: - Gate Status Card

/// Card for backpressure/gate events - build, test, lint status
private struct GateStatusCard: View {
    let event: Event
    @Binding var isExpanded: Bool

    private var isPassing: Bool {
        event.isSuccess
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header
            HStack(spacing: 8) {
                // Status icon
                Image(systemName: isPassing ? "checkmark.circle.fill" : "xmark.circle.fill")
                    .font(.title2)
                    .foregroundColor(isPassing ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentRed)

                VStack(alignment: .leading, spacing: 2) {
                    Text(isPassing ? "GATE PASSED" : "GATE BLOCKED")
                        .font(.system(.caption2, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .kerning(1)

                    Text(event.humanReadableTitle)
                        .font(.system(.subheadline, design: .monospaced).bold())
                        .foregroundColor(isPassing ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentRed)
                }

                Spacer()

                Text(formatTime(event.timestamp))
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            // Summary or error message
            if let summary = event.smartSummary {
                Text(summary)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .lineLimit(isExpanded ? nil : 2)
            }

            // Expand button for long content
            if let payload = event.payload, payload.count > 200 {
                Button {
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                        isExpanded.toggle()
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        Text(isExpanded ? "Show Less" : "Show Full (\(formatSize(payload.count)))")
                    }
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }

                if isExpanded {
                    ScrollView {
                        Text(payload)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.textSecondary)
                            .textSelection(.enabled)
                    }
                    .frame(maxHeight: 300)
                    .padding(8)
                    .background(CyberpunkTheme.bgPrimary)
                    .cornerRadius(8)
                }
            }
        }
        .padding(12)
        .background(isPassing ? CyberpunkTheme.accentGreen.opacity(0.1) : CyberpunkTheme.accentRed.opacity(0.1))
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(isPassing ? CyberpunkTheme.accentGreen.opacity(0.4) : CyberpunkTheme.accentRed.opacity(0.4), lineWidth: 1)
        )
    }

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "h:mm:ss a"
        return formatter.string(from: date)
    }

    private func formatSize(_ count: Int) -> String {
        if count >= 1000 {
            return String(format: "%.1fK", Double(count) / 1000)
        }
        return "\(count) chars"
    }
}

// MARK: - Task Lifecycle Card

/// Card for task lifecycle events - start, complete, terminate
private struct TaskLifecycleCard: View {
    let event: Event
    @Binding var isExpanded: Bool

    private var isCompletion: Bool {
        let topic = event.topic?.lowercased() ?? ""
        return topic.contains("complete") || topic.contains("terminate") || topic.contains("done")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header
            HStack(spacing: 8) {
                // Icon
                Image(systemName: isCompletion ? "flag.checkered" : "target")
                    .font(.title3)
                    .foregroundColor(isCompletion ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentCyan)
                    .frame(width: 28, height: 28)
                    .background((isCompletion ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentCyan).opacity(0.2))
                    .clipShape(Circle())

                VStack(alignment: .leading, spacing: 2) {
                    Text(event.humanReadableTitle.uppercased())
                        .font(.system(.caption2, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .kerning(1)

                    if let iteration = event.iteration {
                        Text("Iteration \(iteration)")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.accentPurple)
                    }
                }

                Spacer()

                Text(formatTime(event.timestamp))
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            // Smart summary
            if let summary = event.smartSummary {
                HStack(alignment: .top, spacing: 6) {
                    Image(systemName: "doc.text")
                        .font(.caption2)
                        .foregroundColor(CyberpunkTheme.textMuted)

                    Text(summary)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textSecondary)
                        .lineLimit(isExpanded ? nil : 3)
                }
            }

            // Expand for full payload
            if let payload = event.payload, payload.count > 300 {
                Button {
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                        isExpanded.toggle()
                    }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        Text(isExpanded ? "Collapse" : "Show Full Spec (\(formatSize(payload.count)))")
                    }
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }

                if isExpanded {
                    ScrollView {
                        Text(payload)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.textSecondary)
                            .textSelection(.enabled)
                    }
                    .frame(maxHeight: 400)
                    .padding(8)
                    .background(CyberpunkTheme.bgPrimary)
                    .cornerRadius(8)
                }
            }
        }
        .padding(12)
        .background(CyberpunkTheme.bgTertiary)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(CyberpunkTheme.accentCyan.opacity(0.3), lineWidth: 1)
        )
    }

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "h:mm:ss a"
        return formatter.string(from: date)
    }

    private func formatSize(_ count: Int) -> String {
        if count >= 1000 {
            return String(format: "%.1fK", Double(count) / 1000)
        }
        return "\(count) chars"
    }
}

// MARK: - Tool Call Row

/// Compact row for tool calls - designed to be scannable
struct ToolCallRow: View {
    let event: Event
    @Binding var isExpanded: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Compact row
            HStack(spacing: 8) {
                // Timestamp
                Text(formatTime(event.timestamp))
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .frame(width: 70, alignment: .leading)

                // Tool badge
                HStack(spacing: 3) {
                    Image(systemName: "wrench.and.screwdriver")
                        .font(.system(size: 9))
                    Text(event.toolName ?? "Tool")
                        .font(.system(.caption, design: .monospaced).bold())
                }
                .foregroundColor(CyberpunkTheme.accentCyan)
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(CyberpunkTheme.accentCyan.opacity(0.15))
                .cornerRadius(4)

                // Target (file path or pattern)
                Text(toolTarget)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .lineLimit(1)
                    .truncationMode(.middle)

                Spacer()

                // Status
                if let status = event.status {
                    statusBadge(status)
                }

                // Duration
                if let duration = event.duration {
                    Text("\(duration)ms")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .frame(width: 50, alignment: .trailing)
                }

                // Expand button
                Button {
                    withAnimation(.spring(response: 0.2, dampingFraction: 0.8)) {
                        isExpanded.toggle()
                    }
                } label: {
                    Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        .font(.caption2)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)

            // Expanded details
            if isExpanded {
                VStack(alignment: .leading, spacing: 8) {
                    Divider()
                        .background(CyberpunkTheme.border)

                    // Input
                    if let input = event.input, !input.isEmpty {
                        DetailSection(title: "INPUT", content: formatJson(input))
                    }

                    // Output
                    if let output = event.output, !output.isEmpty {
                        DetailSection(title: "OUTPUT", content: output, maxHeight: 200)
                    }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 8)
            }
        }
        .background(isExpanded ? CyberpunkTheme.bgTertiary : CyberpunkTheme.bgSecondary)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isExpanded ? CyberpunkTheme.accentCyan.opacity(0.3) : CyberpunkTheme.border, lineWidth: 1)
        )
    }

    private var toolTarget: String {
        guard let input = event.input else { return "" }

        // Common input patterns
        if let path = input["file_path"] as? String {
            return shortenPath(path)
        }
        if let pattern = input["pattern"] as? String {
            return pattern
        }
        if let command = input["command"] as? String {
            return String(command.prefix(40))
        }
        if let query = input["query"] as? String {
            return String(query.prefix(40))
        }

        return ""
    }

    private func shortenPath(_ path: String) -> String {
        let components = path.split(separator: "/")
        if components.count > 3 {
            return ".../" + components.suffix(2).joined(separator: "/")
        }
        return path
    }

    private func statusBadge(_ status: String) -> some View {
        let (color, icon) = statusStyle(status)
        return HStack(spacing: 2) {
            Image(systemName: icon)
                .font(.system(size: 8))
            Text(status)
                .font(.system(.caption2, design: .monospaced))
        }
        .foregroundColor(color)
        .padding(.horizontal, 4)
        .padding(.vertical, 2)
        .background(color.opacity(0.15))
        .cornerRadius(3)
    }

    private func statusStyle(_ status: String) -> (Color, String) {
        switch status.lowercased() {
        case "running": return (CyberpunkTheme.accentYellow, "arrow.triangle.2.circlepath")
        case "completed": return (CyberpunkTheme.accentGreen, "checkmark")
        case "error": return (CyberpunkTheme.accentRed, "xmark")
        case "pending": return (CyberpunkTheme.textMuted, "clock")
        default: return (CyberpunkTheme.textMuted, "questionmark")
        }
    }

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "HH:mm:ss"
        return formatter.string(from: date)
    }

    private func formatJson(_ dict: [String: Any]) -> String {
        do {
            let data = try JSONSerialization.data(withJSONObject: dict, options: [.prettyPrinted, .sortedKeys])
            return String(data: data, encoding: .utf8) ?? "{}"
        } catch {
            return String(describing: dict)
        }
    }
}

// MARK: - Debug Event Row

/// Minimal row for debug/progress events
private struct DebugEventRow: View {
    let event: Event

    var body: some View {
        HStack(spacing: 8) {
            Text(formatTime(event.timestamp))
                .font(.system(.caption2, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)

            Circle()
                .fill(CyberpunkTheme.textMuted)
                .frame(width: 4, height: 4)

            Text(event.topic ?? event.type)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)

            if let payload = event.payload, !payload.isEmpty {
                Text("—")
                    .foregroundColor(CyberpunkTheme.textMuted)
                Text(payload)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .lineLimit(1)
            }

            Spacer()
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .opacity(0.7)
    }

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "HH:mm:ss"
        return formatter.string(from: date)
    }
}

// MARK: - Detail Section

private struct DetailSection: View {
    let title: String
    let content: String
    var maxHeight: CGFloat = 150

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(title)
                    .font(.system(.caption2, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)

                Spacer()

                Button {
                    UIPasteboard.general.string = content
                } label: {
                    Image(systemName: "doc.on.doc")
                        .font(.caption2)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }

            ScrollView(.horizontal, showsIndicators: false) {
                Text(content)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .textSelection(.enabled)
            }
            .frame(maxHeight: maxHeight)
            .padding(8)
            .background(CyberpunkTheme.bgPrimary)
            .cornerRadius(6)
        }
    }
}

// MARK: - Previews

#Preview("Hat Transition") {
    VStack(spacing: 12) {
        VerboseEventRowView(event: Event(
            timestamp: Date(),
            topic: "hat.activated",
            iteration: 5,
            hat: "builder",
            triggered: "planner",
            type: "hat.activated"
        ))

        VerboseEventRowView(event: Event(
            timestamp: Date().addingTimeInterval(-60),
            topic: "hat.activated",
            iteration: 4,
            hat: "planner",
            type: "hat.activated"
        ))
    }
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}

#Preview("Gate Events") {
    VStack(spacing: 12) {
        VerboseEventRowView(event: Event(
            timestamp: Date(),
            topic: "build.passed",
            payload: "All 38 tests passing. Build completed in 4.5s.",
            type: "backpressure"
        ))

        VerboseEventRowView(event: Event(
            timestamp: Date().addingTimeInterval(-30),
            topic: "typecheck.blocked",
            payload: "Cannot find type 'UserService' in scope at src/services/auth.swift:42\n\nFull error output:\nerror: Cannot find type 'UserService' in scope...",
            type: "backpressure"
        ))
    }
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}

#Preview("Task Lifecycle") {
    VStack(spacing: 12) {
        VerboseEventRowView(event: Event(
            timestamp: Date(),
            topic: "task.start",
            payload: "# Task: Build REST API\n\n## Objective\n\nCreate a complete REST API for user management with authentication, CRUD operations, and role-based access control.\n\n## Requirements\n- JWT authentication\n- User registration and login\n- Password reset flow",
            iteration: 1,
            type: "event.published"
        ))

        VerboseEventRowView(event: Event(
            timestamp: Date().addingTimeInterval(-600),
            topic: "loop.terminate",
            payload: "## Reason\ncompleted\n\n## Status\nAll tasks completed successfully.\n\n## Summary\n- Iterations: 5\n- Duration: 12m 51s\n- Exit code: 0",
            iteration: 5,
            type: "event.published"
        ))
    }
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}

#Preview("Tool Calls") {
    VStack(spacing: 8) {
        VerboseEventRowView(event: Event(
            timestamp: Date(),
            type: "tool.call",
            toolName: "Read",
            status: "completed",
            input: ["file_path": "/Users/nick/project/src/main.swift"],
            output: "import Foundation\n\nfunc main() {\n    print(\"Hello\")\n}",
            duration: 45
        ))

        VerboseEventRowView(event: Event(
            timestamp: Date().addingTimeInterval(-5),
            type: "tool.call",
            toolName: "Glob",
            status: "completed",
            input: ["pattern": "**/*.swift"],
            output: "Found 42 files",
            duration: 120
        ))

        VerboseEventRowView(event: Event(
            timestamp: Date().addingTimeInterval(-10),
            type: "tool.call",
            toolName: "Edit",
            status: "running",
            input: ["file_path": "/src/main.swift", "old_string": "Hello", "new_string": "Hello, World!"]
        ))
    }
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}

#Preview("Debug Events") {
    VStack(spacing: 4) {
        VerboseEventRowView(event: Event(
            timestamp: Date(),
            topic: "debug.progress",
            payload: "Processing iteration 5",
            type: "event.published"
        ))

        VerboseEventRowView(event: Event(
            timestamp: Date().addingTimeInterval(-2),
            topic: "test.streaming.1",
            payload: "Live event #1",
            type: "event.published"
        ))
    }
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}
