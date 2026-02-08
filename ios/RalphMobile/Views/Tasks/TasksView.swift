import SwiftUI

/// View for managing Ralph orchestration tasks with filtering and CRUD.
struct TasksView: View {
    @StateObject private var viewModel = TasksViewModel()
    @State private var showCreateSheet = false

    var body: some View {
        VStack(spacing: 0) {
            headerView
            filterBar
            contentView
        }
        .background(CyberpunkTheme.bgPrimary)
        .task {
            await viewModel.fetchTasks()
        }
        .sheet(isPresented: $showCreateSheet) {
            CreateTaskSheet { title, description, priority in
                Task {
                    await viewModel.createTask(title: title, description: description, priority: priority)
                }
                showCreateSheet = false
            }
        }
    }

    // MARK: - Header

    private var headerView: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("TASKS")
                    .font(.system(.headline, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)

                Text("\(viewModel.total) total")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            Spacer()

            Button {
                showCreateSheet = true
            } label: {
                Image(systemName: "plus.circle.fill")
                    .font(.system(size: 28))
                    .foregroundColor(CyberpunkTheme.accentCyan)
            }
            .accessibilityIdentifier("tasks-button-create")
        }
        .padding()
        .background(CyberpunkTheme.bgSecondary)
    }

    // MARK: - Filter Bar

    private var filterBar: some View {
        HStack(spacing: 2) {
            filterButton(title: "All", filter: nil)
            filterButton(title: "Open", filter: "open")
            filterButton(title: "Closed", filter: "closed")
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 6)
        .background(CyberpunkTheme.bgSecondary)
        .overlay(
            Rectangle()
                .fill(CyberpunkTheme.border)
                .frame(height: 1),
            alignment: .bottom
        )
    }

    private func filterButton(title: String, filter: String?) -> some View {
        Button {
            viewModel.statusFilter = filter
            Task { await viewModel.fetchTasks() }
        } label: {
            Text(title)
                .font(.system(.caption, design: .monospaced).bold())
                .frame(maxWidth: .infinity)
                .padding(.vertical, 8)
                .background(
                    viewModel.statusFilter == filter
                        ? CyberpunkTheme.accentPurple.opacity(0.2)
                        : Color.clear
                )
                .foregroundColor(
                    viewModel.statusFilter == filter
                        ? CyberpunkTheme.accentPurple
                        : CyberpunkTheme.textMuted
                )
                .cornerRadius(6)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("tasks-filter-\(title.lowercased())")
    }

    // MARK: - Content

    @ViewBuilder
    private var contentView: some View {
        if viewModel.isLoading {
            loadingView
        } else if let error = viewModel.error {
            errorView(error)
        } else if viewModel.tasks.isEmpty {
            emptyView
        } else {
            tasksList
        }
    }

    private var tasksList: some View {
        ScrollView {
            LazyVStack(spacing: 8) {
                ForEach(viewModel.tasks) { task in
                    TaskRowView(task: task) {
                        let newStatus = task.status == "open" ? "closed" : "open"
                        Task { await viewModel.updateTaskStatus(id: task.id, status: newStatus) }
                    }
                    .accessibilityIdentifier("tasks-item-\(task.id)")
                }
            }
            .padding()
        }
        .refreshable {
            await viewModel.fetchTasks()
        }
    }

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)
            Text("Loading tasks...")
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func errorView(_ message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.accentRed)
            Text(message)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)
            Button("Retry") {
                Task { await viewModel.fetchTasks() }
            }
            .font(.system(.body, design: .monospaced).bold())
            .foregroundColor(CyberpunkTheme.accentCyan)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyView: some View {
        VStack(spacing: 16) {
            Image(systemName: "checklist")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.textMuted)
            Text("No tasks found")
                .font(.system(.headline, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
            Text("Create a task to track work items")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Task Row View

private struct TaskRowView: View {
    let task: TaskItem
    let onToggle: () -> Void

    private var priorityColor: Color {
        switch task.priority {
        case 1: return CyberpunkTheme.accentRed
        case 2: return CyberpunkTheme.accentYellow
        case 3: return CyberpunkTheme.accentCyan
        case 4: return CyberpunkTheme.accentPurple
        default: return CyberpunkTheme.textMuted
        }
    }

    var body: some View {
        HStack(spacing: 12) {
            // Priority badge
            Text("P\(task.priority)")
                .font(.system(.caption2, design: .monospaced).bold())
                .foregroundColor(priorityColor)
                .padding(.horizontal, 6)
                .padding(.vertical, 2)
                .background(priorityColor.opacity(0.2))
                .cornerRadius(4)

            // Task info
            VStack(alignment: .leading, spacing: 4) {
                Text(task.title)
                    .font(.system(.body, design: .monospaced))
                    .foregroundColor(
                        task.status == "closed"
                            ? CyberpunkTheme.textMuted
                            : CyberpunkTheme.textPrimary
                    )
                    .strikethrough(task.status == "closed")

                if let desc = task.description, !desc.isEmpty {
                    Text(desc)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .lineLimit(2)
                }
            }

            Spacer()

            // Status toggle
            Button(action: onToggle) {
                Image(systemName: task.status == "closed" ? "checkmark.circle.fill" : "circle")
                    .font(.title3)
                    .foregroundColor(
                        task.status == "closed"
                            ? CyberpunkTheme.accentGreen
                            : CyberpunkTheme.textMuted
                    )
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("task-toggle-\(task.id)")
        }
        .padding(12)
        .background(CyberpunkTheme.bgSecondary)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
    }
}
