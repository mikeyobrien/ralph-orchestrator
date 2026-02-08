import Foundation

/// ViewModel for managing Ralph orchestration tasks (open/closed work items).
@MainActor
class TasksViewModel: ObservableObject {
    @Published var tasks: [TaskItem] = []
    @Published var total: Int = 0
    @Published var isLoading = false
    @Published var error: String?
    @Published var operationResult: String?
    @Published var statusFilter: String? = nil

    var openTasks: [TaskItem] { tasks.filter { $0.status == "open" } }
    var completedTasks: [TaskItem] { tasks.filter { $0.status == "closed" } }

    func fetchTasks() async {
        guard RalphAPIClient.isConfigured else {
            error = "API client not configured"
            return
        }

        isLoading = true
        error = nil

        do {
            let response = try await RalphAPIClient.shared.getTasks(status: statusFilter)
            tasks = response.tasks
            total = response.total
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func createTask(title: String, description: String?, priority: UInt8) async {
        guard RalphAPIClient.isConfigured else { return }

        operationResult = nil
        do {
            let newTask = try await RalphAPIClient.shared.createTask(
                title: title,
                description: description,
                priority: priority
            )
            tasks.insert(newTask, at: 0)
            total += 1
            operationResult = "Task created"
        } catch {
            self.error = error.localizedDescription
        }
    }

    func updateTaskStatus(id: String, status: String) async {
        guard RalphAPIClient.isConfigured else { return }

        operationResult = nil
        do {
            let updated = try await RalphAPIClient.shared.updateTask(id: id, status: status)
            if let index = tasks.firstIndex(where: { $0.id == id }) {
                tasks[index] = updated
            }
            operationResult = "Task updated"
        } catch {
            self.error = error.localizedDescription
        }
    }
}
