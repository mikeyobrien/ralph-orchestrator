import SwiftUI

/// Compact scratchpad content view for inline display in unified Ralph view
/// Shows agent's working memory with task parsing and live indicator
struct ScratchpadContentView: View {
    let content: String?
    let onRefresh: () -> Void

    @State private var isRefreshing: Bool = false

    private var tasks: [SCVTask] {
        guard let content = content else { return [] }
        return parseTasks(from: content)
    }

    private var hasContent: Bool {
        guard let content = content else { return false }
        return !content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if hasContent {
                // Task progress summary
                if !tasks.isEmpty {
                    taskProgressHeader
                }

                // Task list
                if !tasks.isEmpty {
                    taskListView
                }

                // Raw content preview (truncated)
                rawContentPreview

            } else {
                emptyStateView
            }
        }
    }

    // MARK: - Task Progress Header

    private var taskProgressHeader: some View {
        let completed = tasks.filter { $0.status == .completed }.count
        let total = tasks.count
        let progress = total > 0 ? Double(completed) / Double(total) : 0

        return HStack(spacing: 12) {
            // Progress ring
            ZStack {
                Circle()
                    .stroke(CyberpunkTheme.border, lineWidth: 3)

                Circle()
                    .trim(from: 0, to: progress)
                    .stroke(
                        CyberpunkTheme.accentGreen,
                        style: StrokeStyle(lineWidth: 3, lineCap: .round)
                    )
                    .rotationEffect(.degrees(-90))

                Text("\(completed)/\(total)")
                    .font(.system(.caption2, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)
            }
            .frame(width: 44, height: 44)

            VStack(alignment: .leading, spacing: 2) {
                Text("Tasks")
                    .font(.caption.bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)

                Text("\(Int(progress * 100))% complete")
                    .font(.caption2)
                    .foregroundColor(CyberpunkTheme.textSecondary)
            }

            Spacer()

            // Refresh button
            Button {
                refreshContent()
            } label: {
                Image(systemName: isRefreshing ? "arrow.triangle.2.circlepath" : "arrow.clockwise")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .rotationEffect(.degrees(isRefreshing ? 360 : 0))
                    .animation(
                        isRefreshing ? .linear(duration: 1).repeatForever(autoreverses: false) : .default,
                        value: isRefreshing
                    )
            }
        }
        .padding(10)
        .background(CyberpunkTheme.bgPrimary)
        .cornerRadius(8)
    }

    // MARK: - Task List View

    private var taskListView: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(tasks.prefix(8)) { task in
                SCVTaskRow(task: task)
            }

            if tasks.count > 8 {
                Text("+ \(tasks.count - 8) more tasks")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .padding(.leading, 24)
            }
        }
    }

    // MARK: - Raw Content Preview

    private var rawContentPreview: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Raw Content")
                    .font(.caption.bold())
                    .foregroundColor(CyberpunkTheme.textMuted)

                Spacer()

                Text("\(content?.count ?? 0) chars")
                    .font(.caption2.monospaced())
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            Text(truncatedContent)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
                .lineLimit(6)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
                .background(CyberpunkTheme.bgPrimary)
                .cornerRadius(6)
        }
    }

    private var truncatedContent: String {
        guard let content = content else { return "" }
        let lines = content.components(separatedBy: .newlines)
        let preview = lines.prefix(6).joined(separator: "\n")
        if lines.count > 6 {
            return preview + "\n..."
        }
        return preview
    }

    // MARK: - Empty State

    private var emptyStateView: some View {
        VStack(spacing: 8) {
            Image(systemName: "doc.text")
                .font(.title2)
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No scratchpad content")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)

            Button {
                refreshContent()
            } label: {
                Label("Refresh", systemImage: "arrow.clockwise")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.accentCyan)
            }
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    // MARK: - Actions

    private func refreshContent() {
        isRefreshing = true
        onRefresh()

        // Reset after delay
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            isRefreshing = false
        }
    }

    // MARK: - Task Parsing

    private func parseTasks(from content: String) -> [SCVTask] {
        let lines = content.components(separatedBy: .newlines)
        var tasks: [SCVTask] = []

        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if trimmed.hasPrefix("- [x]") || trimmed.hasPrefix("- [X]") {
                let text = trimmed.dropFirst(5).trimmingCharacters(in: .whitespaces)
                tasks.append(SCVTask(text: String(text), status: .completed))
            } else if trimmed.hasPrefix("- [ ]") {
                let text = trimmed.dropFirst(5).trimmingCharacters(in: .whitespaces)
                tasks.append(SCVTask(text: String(text), status: .pending))
            } else if trimmed.hasPrefix("- [~]") {
                let text = trimmed.dropFirst(5).trimmingCharacters(in: .whitespaces)
                tasks.append(SCVTask(text: String(text), status: .cancelled))
            }
        }

        return tasks
    }
}

// MARK: - Task Model (private to avoid conflict with ScratchpadView.ScratchpadTask)

private struct SCVTask: Identifiable {
    let id = UUID()
    let text: String
    let status: SCVTaskStatus

    enum SCVTaskStatus {
        case pending
        case completed
        case cancelled
    }
}

// MARK: - Task Row

private struct SCVTaskRow: View {
    let task: SCVTask

    private var icon: String {
        switch task.status {
        case .completed: return "checkmark.circle.fill"
        case .pending: return "circle"
        case .cancelled: return "xmark.circle"
        }
    }

    private var iconColor: Color {
        switch task.status {
        case .completed: return CyberpunkTheme.accentGreen
        case .pending: return CyberpunkTheme.textMuted
        case .cancelled: return CyberpunkTheme.accentYellow
        }
    }

    private var textColor: Color {
        switch task.status {
        case .completed: return CyberpunkTheme.textSecondary
        case .pending: return CyberpunkTheme.textPrimary
        case .cancelled: return CyberpunkTheme.textMuted
        }
    }

    private var shouldStrikethrough: Bool {
        task.status == .cancelled
    }

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: icon)
                .font(.caption)
                .foregroundColor(iconColor)
                .frame(width: 16)

            Text(task.text)
                .font(.caption)
                .foregroundColor(textColor)
                .strikethrough(shouldStrikethrough)
                .lineLimit(2)

            Spacer()
        }
    }
}

#Preview {
    let sampleContent = """
    # Iteration 7

    ## Tasks
    - [x] Project initialization
    - [x] Setup database models
    - [~] Cancelled task (reason: no longer needed)
    - [ ] User CRUD endpoints
    - [ ] Authentication middleware
    - [ ] API documentation

    ## Notes
    Working on the user service implementation.
    Need to verify database connection settings.
    """

    return ScratchpadContentView(
        content: sampleContent,
        onRefresh: {
            #if DEBUG
            print("Refreshing...")
            #endif
        }
    )
    .padding()
    .background(CyberpunkTheme.bgSecondary)
}
