import SwiftUI

/// Library view with sub-tabs for Configs, Prompts, Skills, and MCP servers
/// Based on V3 architecture specification
struct LibraryView: View {
    @AppStorage("librarySelectedTab") private var selectedTabRaw: String = LibraryTab.configs.rawValue

    private var selectedTab: LibraryTab {
        get { LibraryTab(rawValue: selectedTabRaw) ?? .configs }
    }

    private func selectTab(_ tab: LibraryTab) {
        withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
            selectedTabRaw = tab.rawValue
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            libraryHeader

            // Tab bar
            tabBar

            // Content
            tabContent
        }
        .background(CyberpunkTheme.bgPrimary)
    }

    // MARK: - Header

    private var libraryHeader: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Library")
                    .font(.title2.bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)

                Text("Manage your assets")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)
            }
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .frame(maxWidth: .infinity)
        .background(CyberpunkTheme.bgSecondary)
    }

    // MARK: - Tab Bar

    private var tabBar: some View {
        HStack(spacing: 2) {
            ForEach(LibraryTab.allCases) { tab in
                Button {
                    selectTab(tab)
                } label: {
                    VStack(spacing: 4) {
                        Image(systemName: tab.icon)
                            .font(.subheadline)

                        Text(tab.title)
                            .font(.caption2)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .background(
                        selectedTab == tab
                            ? CyberpunkTheme.accentPurple.opacity(0.2)
                            : Color.clear
                    )
                    .foregroundColor(
                        selectedTab == tab
                            ? CyberpunkTheme.accentPurple
                            : CyberpunkTheme.textMuted
                    )
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("library-tab-\(tab.rawValue)")
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(CyberpunkTheme.bgSecondary)
        .overlay(
            Rectangle()
                .fill(CyberpunkTheme.border)
                .frame(height: 1),
            alignment: .bottom
        )
    }

    // MARK: - Tab Content

    @ViewBuilder
    private var tabContent: some View {
        switch selectedTab {
        case .configs:
            ConfigsListView()
        case .prompts:
            PromptsListView()
        case .skills:
            SkillsListView()
        case .hats:
            HatsListView()
        case .presets:
            PresetsListView()
        case .memories:
            MemoriesView()
        }
    }
}

// MARK: - Library Tab Enum

enum LibraryTab: String, CaseIterable, Identifiable {
    case configs
    case prompts
    case skills
    case hats
    case presets
    case memories

    var id: String { rawValue }

    var title: String {
        switch self {
        case .configs: return "Configs"
        case .prompts: return "Prompts"
        case .skills: return "Skills"
        case .hats: return "Hats"
        case .presets: return "Presets"
        case .memories: return "Memories"
        }
    }

    var icon: String {
        switch self {
        case .configs: return "doc.text"
        case .prompts: return "text.bubble"
        case .skills: return "sparkles"
        case .hats: return "theatermask.and.paintbrush"
        case .presets: return "slider.horizontal.3"
        case .memories: return "brain"
        }
    }
}

// MARK: - Configs List View

struct ConfigsListView: View {
    @State private var configs: [Config] = []
    @State private var isLoading: Bool = false
    @State private var errorMessage: String?

    var body: some View {
        Group {
            if isLoading {
                loadingView
            } else if let error = errorMessage {
                errorView(error)
            } else if configs.isEmpty {
                emptyView
            } else {
                configsList
            }
        }
        .task {
            await loadConfigs()
        }
    }

    private var loadingView: some View {
        VStack(spacing: 12) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
            Text("Loading configs...")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private func errorView(_ message: String) -> some View {
        VStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
                .font(.title)
                .foregroundColor(CyberpunkTheme.accentYellow)

            Text(message)
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)

            Button("Retry") {
                Task { await loadConfigs() }
            }
            .font(.caption.bold())
            .foregroundColor(CyberpunkTheme.accentCyan)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var emptyView: some View {
        VStack(spacing: 12) {
            Image(systemName: "doc.text")
                .font(.title)
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No configs found")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var configsList: some View {
        ScrollView {
            LazyVStack(spacing: 8) {
                ForEach(configs) { config in
                    APIConfigCard(config: config)
                }
            }
            .padding()
        }
        .accessibilityIdentifier("library-list-view")
    }

    private func loadConfigs() async {
        guard RalphAPIClient.isConfigured else {
            errorMessage = "API client not configured"
            return
        }

        isLoading = true
        errorMessage = nil

        do {
            configs = try await RalphAPIClient.shared.getConfigs()
            isLoading = false
        } catch {
            isLoading = false
            errorMessage = error.localizedDescription
        }
    }
}

// MARK: - API Config Card (uses real Config model)

private struct APIConfigCard: View {
    let config: Config

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(config.name)
                    .font(.system(.subheadline, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)

                Spacer()

                // Preset badge for presets/ paths
                if config.path.hasPrefix("presets/") {
                    Text("preset")
                        .font(.caption2)
                        .foregroundColor(CyberpunkTheme.accentPurple)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(CyberpunkTheme.accentPurple.opacity(0.2))
                        .cornerRadius(4)
                }

                // Action buttons
                HStack(spacing: 8) {
                    Button {
                        UIPasteboard.general.string = config.path
                    } label: {
                        Image(systemName: "doc.on.doc")
                            .font(.caption)
                            .foregroundColor(CyberpunkTheme.textMuted)
                    }
                }
            }

            if !config.description.isEmpty {
                Text(config.description)
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            // Path
            Text(config.path)
                .font(.caption2)
                .foregroundColor(CyberpunkTheme.textMuted.opacity(0.6))
        }
        .padding(12)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
    }
}


// MARK: - Prompts List View

struct PromptsListView: View {
    @State private var prompts: [Prompt] = []
    @State private var isLoading: Bool = false
    @State private var errorMessage: String?
    @State private var selectedPrompt: Prompt?
    @State private var promptContent: String = ""
    @State private var showEditor: Bool = false

    var body: some View {
        Group {
            if isLoading {
                loadingView
            } else if let error = errorMessage {
                errorView(error)
            } else if prompts.isEmpty {
                emptyView
            } else {
                promptsList
            }
        }
        .task {
            await loadPrompts()
        }
    }

    private var loadingView: some View {
        VStack(spacing: 12) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
            Text("Loading prompts...")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private func errorView(_ message: String) -> some View {
        VStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle")
                .font(.title)
                .foregroundColor(CyberpunkTheme.accentYellow)

            Text(message)
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)

            Button("Retry") {
                Task { await loadPrompts() }
            }
            .font(.caption.bold())
            .foregroundColor(CyberpunkTheme.accentCyan)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var emptyView: some View {
        VStack(spacing: 12) {
            Image(systemName: "text.bubble")
                .font(.title)
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No prompts found")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var promptsList: some View {
        ScrollView {
            LazyVStack(spacing: 8) {
                ForEach(prompts) { prompt in
                    APIPromptCard(prompt: prompt)
                        .onTapGesture {
                            selectedPrompt = prompt
                            Task {
                                if let response = try? await RalphAPIClient.shared.getPromptContent(path: prompt.path) {
                                    promptContent = response.content
                                    showEditor = true
                                }
                            }
                        }
                }
            }
            .padding()
        }
        .sheet(isPresented: $showEditor) {
            MarkdownEditorView(
                promptText: $promptContent,
                onTemplatesTap: nil,
                onSave: { content in
                    #if DEBUG
                    print("Saved prompt: \(content.prefix(50))...")
                    #endif
                }
            )
        }
    }

    private func loadPrompts() async {
        guard RalphAPIClient.isConfigured else {
            errorMessage = "API client not configured"
            return
        }

        isLoading = true
        errorMessage = nil

        do {
            prompts = try await RalphAPIClient.shared.getPrompts()
            isLoading = false
        } catch {
            isLoading = false
            errorMessage = error.localizedDescription
        }
    }
}

// MARK: - API Prompt Card (uses real Prompt model)

private struct APIPromptCard: View {
    let prompt: Prompt

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(prompt.name)
                    .font(.system(.subheadline, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textPrimary)

                Spacer()

                HStack(spacing: 8) {
                    Button {
                        UIPasteboard.general.string = prompt.path
                    } label: {
                        Image(systemName: "doc.on.doc")
                            .font(.caption)
                            .foregroundColor(CyberpunkTheme.textMuted)
                    }
                }
            }

            // Preview text
            if !prompt.preview.isEmpty {
                Text(prompt.preview)
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .lineLimit(2)
            }

            // Path
            Text(prompt.path)
                .font(.caption2)
                .foregroundColor(CyberpunkTheme.textMuted.opacity(0.6))
        }
        .padding(12)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
    }
}

// MARK: - Skills List View

struct SkillsListView: View {
    @State private var skills: [Skill] = []
    @State private var isLoading = true
    @State private var errorMessage: String?
    @State private var selectedSkill: Skill?

    var body: some View {
        Group {
            if isLoading {
                VStack(spacing: 12) {
                    ProgressView()
                        .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                    Text("Loading skills...")
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
                .frame(maxWidth: .infinity, minHeight: 100)
            } else if let error = errorMessage {
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.title2)
                        .foregroundColor(CyberpunkTheme.accentYellow)
                    Text(error)
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textSecondary)
                        .multilineTextAlignment(.center)
                    Button("Retry") {
                        Task { await loadSkills() }
                    }
                    .font(.caption.bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }
                .frame(maxWidth: .infinity, minHeight: 100)
            } else if skills.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "puzzlepiece")
                        .font(.title2)
                        .foregroundColor(CyberpunkTheme.textMuted)
                    Text("No skills available")
                        .font(.system(.body, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }
                .frame(maxWidth: .infinity, minHeight: 100)
            } else {
                ScrollView {
                    LazyVStack(spacing: 8) {
                        ForEach(skills) { skill in
                            Button {
                                selectedSkill = skill
                            } label: {
                                HStack(spacing: 12) {
                                    Image(systemName: "puzzlepiece.fill")
                                        .foregroundColor(CyberpunkTheme.accentPurple)
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text(skill.name)
                                            .font(.system(.body, design: .monospaced).bold())
                                            .foregroundColor(CyberpunkTheme.textPrimary)
                                        if !skill.description.isEmpty {
                                            Text(skill.description)
                                                .font(.caption)
                                                .foregroundColor(CyberpunkTheme.textSecondary)
                                                .lineLimit(2)
                                        }
                                    }
                                    Spacer()
                                    Image(systemName: "chevron.right")
                                        .font(.caption)
                                        .foregroundColor(CyberpunkTheme.textMuted)
                                }
                                .padding(12)
                                .background(CyberpunkTheme.bgTertiary)
                                .cornerRadius(8)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding()
                }
            }
        }
        .task {
            await loadSkills()
        }
        .sheet(item: $selectedSkill) { skill in
            SkillDetailView(skill: skill)
        }
    }

    private func loadSkills() async {
        isLoading = true
        errorMessage = nil
        do {
            skills = try await RalphAPIClient.shared.getSkills()
        } catch {
            errorMessage = "Failed to load skills: \(error.localizedDescription)"
        }
        isLoading = false
    }
}

// MARK: - Cyberpunk Toggle Style

struct CyberpunkToggleStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        Button {
            configuration.isOn.toggle()
        } label: {
            RoundedRectangle(cornerRadius: 8)
                .fill(configuration.isOn ? CyberpunkTheme.accentGreen.opacity(0.4) : CyberpunkTheme.bgHover)
                .frame(width: 36, height: 20)
                .overlay(
                    Circle()
                        .fill(configuration.isOn ? CyberpunkTheme.accentGreen : CyberpunkTheme.textMuted)
                        .frame(width: 14, height: 14)
                        .offset(x: configuration.isOn ? 7 : -7)
                        .animation(.spring(response: 0.2, dampingFraction: 0.8), value: configuration.isOn)
                )
        }
        .buttonStyle(.plain)
    }
}

#Preview {
    LibraryView()
}
