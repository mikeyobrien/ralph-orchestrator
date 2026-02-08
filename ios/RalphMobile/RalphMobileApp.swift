import SwiftUI

@main
struct RalphMobileApp: App {
    @StateObject private var navigationManager = NavigationManager()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(navigationManager)
                .onOpenURL { url in
                    handleDeepLink(url)
                }
        }
    }

    private func handleDeepLink(_ url: URL) {
        guard url.scheme == "ralph" else { return }

        switch url.host {
        case "session":
            if let sessionId = url.pathComponents.dropFirst().first {
                navigationManager.navigateToSession(sessionId: sessionId)
            }
        case "settings":
            navigationManager.navigateToSettings()
        case "library":
            navigationManager.navigateToLibrary()
        case "skills":
            navigationManager.navigateToSkills()
        case "host":
            navigationManager.navigateToHost()
        case "tasks":
            navigationManager.navigateToTasks()
        case "loops":
            navigationManager.navigateToLoops()
        case "robot":
            navigationManager.navigateToRobot()
        default:
            break
        }
    }
}

class NavigationManager: ObservableObject {
    @Published var selectedTab: Int = 0
    @Published var sessionNavigationPath: [String] = []
    @Published var libraryNavigationPath: [String] = []
    @Published var selectedGlobalView: String? = nil
    @Published var showCreateWizard: Bool = false

    func navigateToSession(sessionId: String) {
        selectedTab = 0
        sessionNavigationPath = [sessionId]
        selectedGlobalView = nil
    }

    func navigateToSettings() {
        selectedGlobalView = "settings"
    }

    func navigateToLibrary() {
        selectedGlobalView = "library"
    }

    func navigateToSkills() {
        selectedGlobalView = "skills"
    }

    func navigateToHost() {
        selectedGlobalView = "host"
    }

    func navigateToTasks() {
        selectedGlobalView = "tasks"
    }

    func navigateToLoops() {
        selectedGlobalView = "loops"
    }

    func navigateToRobot() {
        selectedGlobalView = "robot"
    }
}
