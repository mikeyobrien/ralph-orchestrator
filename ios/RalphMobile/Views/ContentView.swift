import SwiftUI

/// Root content view for Ralph Mobile
/// Uses a 3-state model: loading → onboarding (if server unreachable) → connected
/// IMPORTANT: Once .connected is reached, MainNavigationView persists forever.
/// Background/settings changes only reconfigure the API client, never touch appState.
struct ContentView: View {
    enum AppState {
        case loading
        case onboarding
        case connected
    }

    @AppStorage("serverURL") private var serverURLString: String = "http://127.0.0.1:8080"
    @State private var appState: AppState = .loading

    private var serverURL: URL {
        URL(string: serverURLString) ?? URL(string: "http://127.0.0.1:8080")!
    }

    var body: some View {
        ZStack {
            CyberpunkTheme.bgPrimary.ignoresSafeArea()

            switch appState {
            case .loading:
                loadingView
            case .onboarding:
                ServerOnboardingView(
                    serverURLString: $serverURLString,
                    onConnected: {
                        withAnimation(.easeInOut(duration: 0.3)) {
                            appState = .connected
                        }
                    }
                )
            case .connected:
                MainNavigationView(serverURL: serverURL, apiKey: "")
            }
        }
        .preferredColorScheme(.dark)
        .task {
            await initialHealthCheck()
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
            // ONLY reconfigure API client — NEVER touch appState (prevents P0 black screen bug)
            Task {
                await reconfigureAPIClient()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .serverCredentialsDidChange)) { _ in
            // ONLY reconfigure API client — NEVER touch appState
            Task {
                await reconfigureAPIClient()
            }
        }
    }

    // MARK: - Health Check

    private func initialHealthCheck() async {
        RalphAPIClient.configure(baseURL: serverURL, apiKey: "")
        let healthy = await RalphAPIClient.checkHealth()
        withAnimation(.easeInOut(duration: 0.3)) {
            appState = healthy ? .connected : .onboarding
        }
    }

    private func reconfigureAPIClient() async {
        RalphAPIClient.configure(baseURL: serverURL, apiKey: "")
    }

    // MARK: - Loading View

    private var loadingView: some View {
        VStack(spacing: 24) {
            VStack(spacing: 12) {
                Image(systemName: "bolt.circle.fill")
                    .font(.system(size: 64))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .shadow(color: CyberpunkTheme.accentCyan.opacity(0.5), radius: 16)

                Text("RALPH MOBILE")
                    .font(.system(.title, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)
                    .kerning(4)

                Text("ORCHESTRATOR")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .kerning(2)
            }

            VStack(spacing: 12) {
                ProgressView()
                    .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                    .scaleEffect(1.5)

                Text("Connecting...")
                    .font(.system(.body, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
            }
        }
    }
}

#Preview {
    ContentView()
}
