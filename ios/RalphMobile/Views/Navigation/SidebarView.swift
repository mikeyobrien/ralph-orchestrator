import SwiftUI

/// Sidebar navigation for Ralph Mobile v5
/// Contains Ralph sessions (top) and global navigation (bottom)
struct SidebarView: View {
    @ObservedObject var viewModel: SessionListViewModel
    @Binding var selectedSession: Session?
    @Binding var selectedGlobalView: GlobalView?
    @Binding var showCreateRalph: Bool

    var body: some View {
        VStack(spacing: 0) {
            // Header
            sidebarHeader

            Divider()
                .background(CyberpunkTheme.border)

            // Ralph sessions list
            sessionsList

            Spacer()

            Divider()
                .background(CyberpunkTheme.border)

            // Global navigation
            globalNavigation
        }
        .background(CyberpunkTheme.bgSecondary)
    }

    // MARK: - Header

    private var sidebarHeader: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("RALPH")
                    .font(.system(.headline, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)

                HStack(spacing: 4) {
                    Circle()
                        .fill(viewModel.isServerReachable ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentRed)
                        .frame(width: 6, height: 6)

                    Text("v\(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?.?.?")")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }

            Spacer()

            // Active session count
            if viewModel.activeSessionCount > 0 {
                HStack(spacing: 4) {
                    Circle()
                        .fill(CyberpunkTheme.accentCyan)
                        .frame(width: 6, height: 6)
                        .pulsing()

                    Text("\(viewModel.activeSessionCount) active")
                        .font(.caption2)
                        .foregroundColor(CyberpunkTheme.accentCyan)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(CyberpunkTheme.accentCyan.opacity(0.15))
                .cornerRadius(4)
            }
        }
        .padding()
    }

    // MARK: - Sessions List

    private var sessionsList: some View {
        ZStack {
            if let error = viewModel.error {
                // Show error state
                errorView(error: error)
            } else if viewModel.isLoading && viewModel.sessions.isEmpty {
                // Show loading indicator on initial load
                loadingView
            } else if viewModel.sessions.isEmpty && !viewModel.isLoading {
                // Show empty state after loading completes
                emptyStateView
            } else {
                // Show sessions list
                ScrollView {
                    LazyVStack(spacing: 4) {
                        // New Ralph button
                        newRalphButton

                        // Session items
                        ForEach(viewModel.sessions) { session in
                            SessionSidebarItem(
                                session: session,
                                isSelected: selectedSession?.id == session.id,
                                onTap: {
                                    selectedSession = session
                                    selectedGlobalView = nil
                                }
                            )
                        }
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                }
            }
        }
        .accessibilityIdentifier("sidebar-session-list")
    }

    // MARK: - Loading View

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)

            Text("Loading sessions...")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Empty State View

    private var emptyStateView: some View {
        VStack(spacing: 16) {
            Image(systemName: "tray")
                .font(.system(size: 40))
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No sessions")
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)

            Text("Create a new Ralph to get started")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    // MARK: - Error View

    private func errorView(error: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 40))
                .foregroundColor(CyberpunkTheme.accentRed)

            Text("Connection Error")
                .font(.system(.body, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textPrimary)

            Text(error)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)
                .padding(.horizontal)

            Button {
                Task {
                    await viewModel.fetchSessions()
                }
            } label: {
                HStack {
                    Image(systemName: "arrow.clockwise")
                    Text("Retry")
                }
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.bgPrimary)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(CyberpunkTheme.accentCyan)
                .cornerRadius(6)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var newRalphButton: some View {
        Button {
            showCreateRalph = true
        } label: {
            HStack(spacing: 8) {
                ZStack {
                    RoundedRectangle(cornerRadius: 6)
                        .fill(CyberpunkTheme.accentCyan.opacity(0.2))
                        .frame(width: 32, height: 32)

                    Image(systemName: "plus")
                        .font(.system(.body, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.accentCyan)
                }

                Text("New Ralph")
                    .font(.subheadline)
                    .foregroundColor(CyberpunkTheme.textPrimary)

                Spacer()
            }
            .padding(8)
            .background(CyberpunkTheme.bgTertiary)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(CyberpunkTheme.accentCyan.opacity(0.3), lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("sidebar-button-new-session")
    }

    // MARK: - Global Navigation

    private var globalNavigation: some View {
        VStack(spacing: 4) {
            ForEach(GlobalView.allCases) { view in
                GlobalNavItem(
                    view: view,
                    isSelected: selectedGlobalView == view,
                    onTap: {
                        selectedGlobalView = view
                        selectedSession = nil
                    }
                )
                .accessibilityIdentifier("sidebar-tab-\(view.rawValue)")
            }
        }
        .padding(8)
    }
}

// MARK: - Global View Enum

enum GlobalView: String, CaseIterable, Identifiable {
    case skills = "skills"
    case library = "library"
    case tasks = "tasks"
    case loops = "loops"
    case robot = "robot"
    case host = "host"
    case settings = "settings"

    var id: String { rawValue }

    var icon: String {
        switch self {
        case .skills: return "cube.fill"
        case .library: return "books.vertical"
        case .tasks: return "checklist"
        case .loops: return "arrow.triangle.branch"
        case .robot: return "bubble.left.and.bubble.right"
        case .host: return "gauge.open.with.lines.needle.33percent"
        case .settings: return "gearshape"
        }
    }

    var title: String {
        switch self {
        case .skills: return "Skills"
        case .library: return "Library"
        case .tasks: return "Tasks"
        case .loops: return "Loops"
        case .robot: return "RObot"
        case .host: return "Host Metrics"
        case .settings: return "Settings"
        }
    }
}

// MARK: - Session Sidebar Item

private struct SessionSidebarItem: View {
    let session: Session
    let isSelected: Bool
    let onTap: () -> Void

    private var statusColor: Color {
        switch session.status {
        case "running": return CyberpunkTheme.accentCyan
        case "paused": return CyberpunkTheme.accentYellow
        case "completed", "complete": return CyberpunkTheme.accentGreen
        case "error", "failed": return CyberpunkTheme.accentRed
        default: return CyberpunkTheme.textMuted
        }
    }

    private var firstLetter: String {
        String(session.id.prefix(1).uppercased())
    }

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 8) {
                // Session icon with status dot
                ZStack(alignment: .bottomTrailing) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 6)
                            .fill(isSelected ? statusColor.opacity(0.3) : CyberpunkTheme.bgTertiary)
                            .frame(width: 32, height: 32)

                        Text(firstLetter)
                            .font(.system(.subheadline, design: .monospaced).bold())
                            .foregroundColor(isSelected ? statusColor : CyberpunkTheme.textSecondary)
                    }

                    // Status dot
                    Circle()
                        .fill(statusColor)
                        .frame(width: 8, height: 8)
                        .overlay(
                            Circle()
                                .stroke(CyberpunkTheme.bgSecondary, lineWidth: 2)
                        )
                        .offset(x: 2, y: 2)
                        .pulsing(session.status == "running")
                }

                // Session name
                VStack(alignment: .leading, spacing: 2) {
                    Text(session.id)
                        .font(.subheadline)
                        .foregroundColor(isSelected ? CyberpunkTheme.textPrimary : CyberpunkTheme.textSecondary)
                        .lineLimit(1)

                    if let hat = session.hat, !hat.isEmpty {
                        Text(hat)
                            .font(.caption2)
                            .foregroundColor(CyberpunkTheme.textMuted)
                            .lineLimit(1)
                    }
                }

                Spacer()
            }
            .padding(8)
            .contentShape(Rectangle()) // Ensure entire row is tappable
            .background(isSelected ? statusColor.opacity(0.1) : Color.clear)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(isSelected ? statusColor.opacity(0.5) : Color.clear, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Global Nav Item

private struct GlobalNavItem: View {
    let view: GlobalView
    let isSelected: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 12) {
                Image(systemName: view.icon)
                    .font(.body)
                    .foregroundColor(isSelected ? CyberpunkTheme.accentCyan : CyberpunkTheme.textMuted)
                    .frame(width: 24)

                Text(view.title)
                    .font(.subheadline)
                    .foregroundColor(isSelected ? CyberpunkTheme.textPrimary : CyberpunkTheme.textSecondary)

                Spacer()
            }
            .padding(10)
            .background(isSelected ? CyberpunkTheme.accentCyan.opacity(0.1) : Color.clear)
            .cornerRadius(8)
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Pulsing Modifier

extension View {
    func pulsing(_ active: Bool = true) -> some View {
        modifier(PulsingModifier(isActive: active))
    }
}

private struct PulsingModifier: ViewModifier {
    let isActive: Bool
    @State private var isPulsing = false

    func body(content: Content) -> some View {
        content
            .opacity(isActive && isPulsing ? 0.5 : 1.0)
            .onAppear {
                if isActive {
                    withAnimation(.easeInOut(duration: 1.0).repeatForever(autoreverses: true)) {
                        isPulsing = true
                    }
                }
            }
            .onChange(of: isActive) { active in
                if active {
                    withAnimation(.easeInOut(duration: 1.0).repeatForever(autoreverses: true)) {
                        isPulsing = true
                    }
                } else {
                    isPulsing = false
                }
            }
    }
}

// MARK: - Session List ViewModel

@MainActor
class SessionListViewModel: ObservableObject {
    @Published var sessions: [Session] = []
    @Published var isLoading: Bool = true  // Start as true to show loading on first render
    @Published var error: String? = nil
    @Published var isServerReachable: Bool = true

    var activeSessionCount: Int {
        sessions.filter { $0.status == "running" }.count
    }

    func fetchSessions() async {
        guard RalphAPIClient.isConfigured else {
            error = "API client not configured"
            isLoading = false
            return
        }

        isLoading = true
        error = nil

        do {
            sessions = try await RalphAPIClient.shared.getSessions()
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }

    func checkHealth() async {
        isServerReachable = await RalphAPIClient.checkHealth()
    }
}

#Preview {
    SidebarPreviewWrapper()
}

private struct SidebarPreviewWrapper: View {
    @StateObject private var viewModel = SessionListViewModel()

    var body: some View {
        SidebarView(
            viewModel: viewModel,
            selectedSession: .constant(nil),
            selectedGlobalView: .constant(nil),
            showCreateRalph: .constant(false)
        )
        .frame(width: 250)
        .onAppear {
            viewModel.sessions = [
                Session(
                    id: "api-service",
                    iteration: 3,
                    hat: "builder",
                    status: "running"
                ),
                Session(
                    id: "web-client",
                    iteration: 1,
                    hat: "reviewer",
                    status: "paused"
                ),
                Session(
                    id: "data-pipeline",
                    iteration: 5,
                    hat: nil,
                    status: "completed"
                )
            ]
        }
    }
}
