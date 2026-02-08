import SwiftUI

/// Scratchpad Viewer screen - displays the shared scratchpad state with task progress
/// Matches ScratchpadView from ralph-mobile-ui-documentation.md
struct ScratchpadView: View {
    @ObservedObject var viewModel: SessionViewModel
    @State private var autoRefresh: Bool = true

    // Parse tasks from scratchpad
    private var tasks: [ScratchpadTask] {
        guard let content = viewModel.scratchpadContent else { return [] }
        return parseTasks(from: content)
    }

    private var completedCount: Int {
        tasks.filter { $0.status == .completed }.count
    }

    private var progress: Double {
        guard !tasks.isEmpty else { return 0 }
        return Double(completedCount) / Double(tasks.count)
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                // Header
                headerSection

                // Progress bar
                progressSection

                // Task list or content
                if !tasks.isEmpty {
                    taskListSection
                } else if let content = viewModel.scratchpadContent {
                    rawContentSection(content)
                } else if viewModel.isLoading {
                    loadingView
                } else {
                    emptyScratchpadView
                }
            }
            .padding()
        }
        .background(CyberpunkTheme.bgPrimary)
        .task {
            await viewModel.fetchScratchpad()
        }
    }

    // MARK: - Header Section

    private var headerSection: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("SCRATCHPAD")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .kerning(2)

                HStack(spacing: 8) {
                    Image(systemName: "doc.plaintext.fill")
                        .foregroundColor(CyberpunkTheme.accentYellow)
                    Text(".agent/scratchpad.md")
                        .font(.system(.subheadline, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textPrimary)
                }
            }

            Spacer()

            // Auto-refresh toggle
            HStack(spacing: 8) {
                Toggle("", isOn: $autoRefresh)
                    .toggleStyle(SwitchToggleStyle(tint: CyberpunkTheme.accentCyan))
                    .labelsHidden()

                Text("Auto")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            // Refresh button
            Button {
                Task { await viewModel.fetchScratchpad() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .padding(10)
                    .background(CyberpunkTheme.bgTertiary)
                    .cornerRadius(8)
            }
        }
    }

    // MARK: - Progress Section

    private var progressSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Task Progress")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)

                Spacer()

                Text("\(completedCount)/\(tasks.count)")
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentGreen)

                Text("(\(Int(progress * 100))%)")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            // Progress bar
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    // Background
                    RoundedRectangle(cornerRadius: 4)
                        .fill(CyberpunkTheme.bgTertiary)
                        .frame(height: 8)

                    // Progress fill
                    RoundedRectangle(cornerRadius: 4)
                        .fill(
                            LinearGradient(
                                colors: [CyberpunkTheme.accentCyan, CyberpunkTheme.accentGreen],
                                startPoint: .leading,
                                endPoint: .trailing
                            )
                        )
                        .frame(width: geometry.size.width * progress, height: 8)
                        .shadow(color: CyberpunkTheme.accentCyan.opacity(0.5), radius: 4)
                }
            }
            .frame(height: 8)
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
    }

    // MARK: - Task List Section

    private var taskListSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("TASKS")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .kerning(1)

            ForEach(tasks) { task in
                TaskRow(task: task)
            }
        }
    }

    // MARK: - Raw Content Section

    private func rawContentSection(_ content: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("CONTENT")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .kerning(1)

            ScrollView {
                Text(content)
                    .font(.system(.body, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding()
            .background(CyberpunkTheme.bgCard)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(CyberpunkTheme.border, lineWidth: 1)
            )
        }
    }

    // MARK: - Loading & Empty States

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)

            Text("Loading scratchpad...")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textSecondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 60)
    }

    private var emptyScratchpadView: some View {
        VStack(spacing: 12) {
            Image(systemName: "doc.text.magnifyingglass")
                .font(.system(size: 40))
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No scratchpad content")
                .font(.headline)
                .foregroundColor(CyberpunkTheme.textSecondary)

            Text("Start a session to view task progress")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)

            Button {
                Task { await viewModel.fetchScratchpad() }
            } label: {
                Text("Retry")
                    .font(.subheadline.bold())
                    .foregroundColor(CyberpunkTheme.bgPrimary)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 10)
                    .background(CyberpunkTheme.accentCyan)
                    .cornerRadius(8)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(12)
    }

    // MARK: - Task Parsing

    private func parseTasks(from content: String) -> [ScratchpadTask] {
        var tasks: [ScratchpadTask] = []
        let lines = content.split(separator: "\n")

        for (index, line) in lines.enumerated() {
            let lineStr = String(line).trimmingCharacters(in: .whitespaces)

            // Parse checkbox items: - [x], - [ ], - [~]
            if lineStr.hasPrefix("- [") || lineStr.hasPrefix("* [") {
                let status: TaskStatus
                let title: String

                if lineStr.contains("[x]") || lineStr.contains("[X]") {
                    status = .completed
                    title = lineStr.replacingOccurrences(of: "- [x] ", with: "")
                        .replacingOccurrences(of: "- [X] ", with: "")
                        .replacingOccurrences(of: "* [x] ", with: "")
                        .replacingOccurrences(of: "* [X] ", with: "")
                } else if lineStr.contains("[~]") {
                    status = .inProgress
                    title = lineStr.replacingOccurrences(of: "- [~] ", with: "")
                        .replacingOccurrences(of: "* [~] ", with: "")
                } else {
                    status = .pending
                    title = lineStr.replacingOccurrences(of: "- [ ] ", with: "")
                        .replacingOccurrences(of: "* [ ] ", with: "")
                }

                tasks.append(ScratchpadTask(id: index, title: title, status: status))
            }
            // Also parse ✓, ◐, ○ symbols
            else if lineStr.hasPrefix("✓ ") {
                tasks.append(ScratchpadTask(id: index, title: String(lineStr.dropFirst(2)), status: .completed))
            } else if lineStr.hasPrefix("◐ ") {
                tasks.append(ScratchpadTask(id: index, title: String(lineStr.dropFirst(2)), status: .inProgress))
            } else if lineStr.hasPrefix("○ ") {
                tasks.append(ScratchpadTask(id: index, title: String(lineStr.dropFirst(2)), status: .pending))
            }
        }

        return tasks
    }
}

// MARK: - Task Model

struct ScratchpadTask: Identifiable {
    let id: Int
    let title: String
    let status: TaskStatus
}

enum TaskStatus {
    case pending
    case inProgress
    case completed

    var icon: String {
        switch self {
        case .pending: return "circle"
        case .inProgress: return "circle.lefthalf.filled"
        case .completed: return "checkmark.circle.fill"
        }
    }

    var color: Color {
        switch self {
        case .pending: return CyberpunkTheme.textMuted
        case .inProgress: return CyberpunkTheme.accentYellow
        case .completed: return CyberpunkTheme.accentGreen
        }
    }
}

// MARK: - Task Row

private struct TaskRow: View {
    let task: ScratchpadTask
    @State private var isPulsing: Bool = false

    var body: some View {
        HStack(spacing: 12) {
            // Status icon
            Image(systemName: task.status.icon)
                .font(.system(size: 16))
                .foregroundColor(task.status.color)
                .scaleEffect(isPulsing && task.status == .inProgress ? 1.1 : 1.0)
                .animation(
                    task.status == .inProgress ?
                        .easeInOut(duration: 0.8).repeatForever(autoreverses: true) : .default,
                    value: isPulsing
                )

            // Task title
            Text(task.title)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(task.status == .completed ? CyberpunkTheme.textMuted : CyberpunkTheme.textPrimary)
                .strikethrough(task.status == .completed, color: CyberpunkTheme.textMuted)

            Spacer()
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(
                    task.status == .inProgress ? CyberpunkTheme.accentYellow.opacity(0.5) : CyberpunkTheme.border,
                    lineWidth: 1
                )
        )
        .onAppear {
            if task.status == .inProgress {
                isPulsing = true
            }
        }
    }
}

#Preview {
    NavigationStack {
        ScratchpadView(
            viewModel: SessionViewModel(
                baseURL: URL(string: "http://localhost:8080")!,
                apiKey: ""
            )
        )
    }
}
