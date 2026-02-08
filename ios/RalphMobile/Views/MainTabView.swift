import SwiftUI

/// Main tab navigation view with cyberpunk styling
/// Implements the 5-tab structure from ralph-mobile-ui-documentation.md
struct MainTabView: View {
    @ObservedObject var viewModel: SessionViewModel
    @State private var selectedTab: Tab = .dashboard
    @State private var showSteeringSheet: Bool = false

    enum Tab: String, CaseIterable {
        case dashboard = "Dashboard"
        case stream = "Stream"
        case config = "Config"
        case prompt = "Prompt"
        case scratchpad = "Scratchpad"

        var icon: String {
            switch self {
            case .dashboard: return "house.fill"
            case .stream: return "chart.bar.doc.horizontal"
            case .config: return "gearshape.fill"
            case .prompt: return "pencil.circle.fill"
            case .scratchpad: return "doc.plaintext.fill"
            }
        }

        var color: Color {
            switch self {
            case .dashboard: return CyberpunkTheme.accentCyan
            case .stream: return CyberpunkTheme.accentMagenta
            case .config: return CyberpunkTheme.accentOrange
            case .prompt: return CyberpunkTheme.accentPurple
            case .scratchpad: return CyberpunkTheme.accentYellow
            }
        }
    }

    var body: some View {
        ZStack(alignment: .bottom) {
            // Main content
            Group {
                switch selectedTab {
                case .dashboard:
                    DashboardView(viewModel: viewModel, showSteeringSheet: $showSteeringSheet)
                case .stream:
                    StreamView(viewModel: viewModel)
                case .config:
                    ConfigView(viewModel: viewModel)
                case .prompt:
                    PromptView(viewModel: viewModel)
                case .scratchpad:
                    ScratchpadView(viewModel: viewModel)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            // Cyberpunk tab bar
            cyberpunkTabBar

            // User steering FAB (when session is running)
            if viewModel.currentSession?.status == "running" {
                UserSteeringFAB(isPresented: $showSteeringSheet)
                    .position(x: UIScreen.main.bounds.width - 50, y: UIScreen.main.bounds.height - 180)
            }
        }
        .background(CyberpunkTheme.bgPrimary)
        .sheet(isPresented: $showSteeringSheet) {
            SteeringSheet(viewModel: viewModel, isPresented: $showSteeringSheet)
        }
    }

    // MARK: - Cyberpunk Tab Bar

    private var cyberpunkTabBar: some View {
        HStack(spacing: 0) {
            ForEach(Tab.allCases, id: \.self) { tab in
                tabButton(for: tab)
            }
        }
        .padding(.horizontal, 8)
        .padding(.top, 8)
        .padding(.bottom, 24) // Safe area padding
        .background(
            CyberpunkTheme.bgSecondary
                .overlay(
                    Rectangle()
                        .fill(CyberpunkTheme.border)
                        .frame(height: 1),
                    alignment: .top
                )
        )
    }

    private func tabButton(for tab: Tab) -> some View {
        let isSelected = selectedTab == tab

        return Button {
            withAnimation(.easeInOut(duration: 0.2)) {
                selectedTab = tab
            }
        } label: {
            VStack(spacing: 4) {
                Image(systemName: tab.icon)
                    .font(.system(size: 20))
                    .foregroundColor(isSelected ? tab.color : CyberpunkTheme.textMuted)
                    .shadow(color: isSelected ? tab.color.opacity(0.5) : .clear, radius: 4)

                Text(tab.rawValue)
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(isSelected ? tab.color : CyberpunkTheme.textMuted)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 8)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("main-tab-\(tab.rawValue.lowercased())")
    }
}

// MARK: - Steering Sheet

struct SteeringSheet: View {
    @ObservedObject var viewModel: SessionViewModel
    @Binding var isPresented: Bool
    @State private var message: String = ""
    @State private var isSending: Bool = false
    @FocusState private var isMessageFocused: Bool

    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                // Header
                VStack(spacing: 8) {
                    Image(systemName: "message.fill")
                        .font(.system(size: 32))
                        .foregroundColor(CyberpunkTheme.accentPurple)
                        .shadow(color: CyberpunkTheme.accentPurple.opacity(0.5), radius: 8)

                    Text("STEER SESSION")
                        .font(.system(.title3, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    Text("Send guidance to the running session")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }
                .padding(.top, 20)

                // Message input
                VStack(alignment: .leading, spacing: 8) {
                    Text("MESSAGE")
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .kerning(1)

                    TextEditor(text: $message)
                        .font(.system(.body, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textPrimary)
                        .scrollContentBackground(.hidden)
                        .focused($isMessageFocused)
                        .frame(minHeight: 120)
                        .padding(12)
                        .background(CyberpunkTheme.bgTertiary)
                        .cornerRadius(8)
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .stroke(
                                    isMessageFocused ? CyberpunkTheme.accentPurple : CyberpunkTheme.border,
                                    lineWidth: 1
                                )
                        )
                }
                .padding(.horizontal)

                // Quick suggestions
                VStack(alignment: .leading, spacing: 8) {
                    Text("QUICK SUGGESTIONS")
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .kerning(1)

                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            suggestionChip("Focus on tests first")
                            suggestionChip("Simplify the approach")
                            suggestionChip("Check for edge cases")
                            suggestionChip("Add error handling")
                        }
                    }
                }
                .padding(.horizontal)

                Spacer()

                // Send button
                Button {
                    Task {
                        isSending = true
                        await viewModel.sendSteeringMessage(message)
                        isSending = false
                        message = ""
                        isPresented = false
                    }
                } label: {
                    HStack {
                        if isSending {
                            ProgressView()
                                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.bgPrimary))
                        } else {
                            Image(systemName: "paperplane.fill")
                        }
                        Text(isSending ? "SENDING..." : "SEND GUIDANCE")
                            .font(.system(.body, design: .monospaced).bold())
                    }
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(message.isEmpty ? CyberpunkTheme.textMuted : CyberpunkTheme.accentPurple)
                    .foregroundColor(CyberpunkTheme.bgPrimary)
                    .cornerRadius(12)
                }
                .disabled(message.isEmpty || isSending)
                .padding()
            }
            .background(CyberpunkTheme.bgPrimary)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        isPresented = false
                    }
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }
            }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
        .onAppear {
            isMessageFocused = true
        }
    }

    private func suggestionChip(_ text: String) -> some View {
        Button {
            message = text
        } label: {
            Text(text)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.accentPurple)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(CyberpunkTheme.bgTertiary)
                .cornerRadius(16)
                .overlay(
                    RoundedRectangle(cornerRadius: 16)
                        .stroke(CyberpunkTheme.accentPurple.opacity(0.3), lineWidth: 1)
                )
        }
    }
}

#Preview {
    MainTabView(
        viewModel: SessionViewModel(
            baseURL: URL(string: "http://localhost:8080")!,
            apiKey: ""
        )
    )
}
