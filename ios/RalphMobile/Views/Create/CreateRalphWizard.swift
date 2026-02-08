import SwiftUI

/// 3-step wizard for creating a new Ralph session
/// Step 1: Select Config
/// Step 2: Select Prompt
/// Step 3: Working Directory
struct CreateRalphWizard: View {
    let onComplete: (String, String, String) -> Void
    let onCancel: () -> Void

    @State private var currentStep: Int = 1
    @State private var selectedConfig: ConfigOption? = nil
    @State private var selectedPrompt: PromptOption? = nil
    @State private var directory: String = ""

    @State private var configs: [ConfigOption] = []
    @State private var prompts: [PromptOption] = []
    @State private var isLoadingConfigs: Bool = false
    @State private var isLoadingPrompts: Bool = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // Progress indicator
                progressIndicator

                Divider()
                    .background(CyberpunkTheme.border)

                // Step content
                stepContent

                Divider()
                    .background(CyberpunkTheme.border)

                // Navigation buttons
                navigationButtons
            }
            .background(CyberpunkTheme.bgPrimary)
            .navigationTitle("New Ralph")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        onCancel()
                    }
                    .foregroundColor(CyberpunkTheme.textSecondary)
                }
            }
        }
        .task {
            await loadConfigs()
            await loadPrompts()
        }
    }

    // MARK: - Progress Indicator

    private var progressIndicator: some View {
        HStack(spacing: 0) {
            ForEach(1...3, id: \.self) { step in
                HStack(spacing: 8) {
                    // Step circle
                    ZStack {
                        Circle()
                            .fill(step <= currentStep ? CyberpunkTheme.accentCyan : CyberpunkTheme.bgTertiary)
                            .frame(width: 28, height: 28)

                        if step < currentStep {
                            Image(systemName: "checkmark")
                                .font(.caption.bold())
                                .foregroundColor(CyberpunkTheme.bgPrimary)
                        } else {
                            Text("\(step)")
                                .font(.caption.bold())
                                .foregroundColor(step <= currentStep ? CyberpunkTheme.bgPrimary : CyberpunkTheme.textMuted)
                        }
                    }

                    // Step label
                    Text(stepTitle(for: step))
                        .font(.caption)
                        .foregroundColor(step <= currentStep ? CyberpunkTheme.textPrimary : CyberpunkTheme.textMuted)
                }

                if step < 3 {
                    Spacer()

                    // Connector line
                    Rectangle()
                        .fill(step < currentStep ? CyberpunkTheme.accentCyan : CyberpunkTheme.border)
                        .frame(height: 2)
                        .frame(maxWidth: 40)

                    Spacer()
                }
            }
        }
        .padding()
    }

    private func stepTitle(for step: Int) -> String {
        switch step {
        case 1: return "Config"
        case 2: return "Prompt"
        case 3: return "Directory"
        default: return ""
        }
    }

    // MARK: - Step Content

    @ViewBuilder
    private var stepContent: some View {
        switch currentStep {
        case 1:
            configSelectionStep
        case 2:
            promptSelectionStep
        case 3:
            directoryStep
        default:
            EmptyView()
        }
    }

    // MARK: - Step 1: Config Selection

    private var configSelectionStep: some View {
        ScrollView {
            LazyVStack(spacing: 12) {
                if isLoadingConfigs {
                    ProgressView()
                        .tint(CyberpunkTheme.accentCyan)
                        .padding()
                } else if configs.isEmpty {
                    emptyStateView(title: "No Configs", message: "No configurations available")
                } else {
                    ForEach(configs) { config in
                        ConfigCard(
                            config: config,
                            isSelected: selectedConfig?.id == config.id,
                            onTap: {
                                withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                                    selectedConfig = config
                                }
                            }
                        )
                    }
                }
            }
            .padding()
        }
    }

    // MARK: - Step 2: Prompt Selection

    private var promptSelectionStep: some View {
        ScrollView {
            LazyVStack(spacing: 12) {
                if isLoadingPrompts {
                    ProgressView()
                        .tint(CyberpunkTheme.accentCyan)
                        .padding()
                } else if prompts.isEmpty {
                    emptyStateView(title: "No Prompts", message: "No saved prompts available")
                } else {
                    ForEach(prompts) { prompt in
                        PromptCard(
                            prompt: prompt,
                            isSelected: selectedPrompt?.id == prompt.id,
                            onTap: {
                                withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                                    selectedPrompt = prompt
                                }
                            }
                        )
                    }
                }
            }
            .padding()
        }
    }

    // MARK: - Step 3: Directory

    private var directoryStep: some View {
        ScrollView {
            VStack(spacing: 20) {
                // Directory input
                VStack(alignment: .leading, spacing: 8) {
                    Label("Working Directory", systemImage: "folder")
                        .font(.subheadline.bold())
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    TextField("~/projects/my-project", text: $directory)
                        .textFieldStyle(.plain)
                        .font(.body.monospaced())
                        .foregroundColor(CyberpunkTheme.textPrimary)
                        .padding()
                        .background(CyberpunkTheme.bgTertiary)
                        .cornerRadius(8)
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .stroke(CyberpunkTheme.border, lineWidth: 1)
                        )

                    Text("Enter the full path to your project directory")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }

                Divider()
                    .background(CyberpunkTheme.border)

                // Summary
                summarySection
            }
            .padding()
        }
    }

    // MARK: - Summary Section

    private var summarySection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Summary")
                .font(.headline)
                .foregroundColor(CyberpunkTheme.textPrimary)

            // Config summary
            if let config = selectedConfig {
                SummaryRow(
                    icon: "gearshape",
                    title: "Config",
                    value: config.name,
                    color: CyberpunkTheme.accentPurple
                )
            }

            // Prompt summary
            if let prompt = selectedPrompt {
                SummaryRow(
                    icon: "doc.text",
                    title: "Prompt",
                    value: prompt.name,
                    color: CyberpunkTheme.accentYellow
                )
            }

            // Hat composition
            if let config = selectedConfig {
                SummaryRow(
                    icon: "person.3",
                    title: "Hats",
                    value: config.hats.joined(separator: " → "),
                    color: CyberpunkTheme.accentCyan
                )
            }

            // Directory
            if !directory.isEmpty {
                SummaryRow(
                    icon: "folder",
                    title: "Directory",
                    value: directory,
                    color: CyberpunkTheme.accentGreen
                )
            }
        }
        .padding()
        .background(CyberpunkTheme.bgTertiary)
        .cornerRadius(8)
    }

    // MARK: - Navigation Buttons

    private var navigationButtons: some View {
        HStack(spacing: 12) {
            // Back button
            if currentStep > 1 {
                Button {
                    withAnimation {
                        currentStep -= 1
                    }
                } label: {
                    HStack {
                        Image(systemName: "chevron.left")
                        Text("Back")
                    }
                    .font(.subheadline)
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(CyberpunkTheme.bgTertiary)
                    .cornerRadius(8)
                }
            }

            Spacer()

            // Next/Start button
            Button {
                if currentStep < 3 {
                    withAnimation {
                        currentStep += 1
                    }
                } else {
                    // Create the Ralph
                    onComplete(
                        selectedConfig?.name ?? "",
                        selectedPrompt?.path ?? "",
                        directory
                    )
                }
            } label: {
                HStack {
                    Text(currentStep < 3 ? "Next" : "Start Ralph")
                    if currentStep < 3 {
                        Image(systemName: "chevron.right")
                    } else {
                        Image(systemName: "play.fill")
                    }
                }
                .font(.subheadline.bold())
                .foregroundColor(CyberpunkTheme.bgPrimary)
                .padding(.horizontal, 20)
                .padding(.vertical, 10)
                .background(canProceed ? CyberpunkTheme.accentCyan : CyberpunkTheme.textMuted)
                .cornerRadius(8)
            }
            .disabled(!canProceed)
        }
        .padding()
    }

    private var canProceed: Bool {
        switch currentStep {
        case 1: return selectedConfig != nil
        case 2: return selectedPrompt != nil
        case 3: return !directory.isEmpty
        default: return false
        }
    }

    // MARK: - Empty State

    private func emptyStateView(title: String, message: String) -> some View {
        VStack(spacing: 8) {
            Text(title)
                .font(.headline)
                .foregroundColor(CyberpunkTheme.textSecondary)

            Text(message)
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
    }

    // MARK: - Data Loading

    private func loadConfigs() async {
        isLoadingConfigs = true
        defer { isLoadingConfigs = false }

        do {
            let apiConfigs = try await RalphAPIClient.shared.getConfigs()
            configs = apiConfigs.map { config in
                ConfigOption(
                    id: config.id,
                    name: config.name,
                    description: config.description,
                    hats: [], // Hat composition not in API response
                    isPreset: config.path.hasPrefix("presets/")
                )
            }
        } catch {
            // Graceful fallback: show empty state when API unavailable
            configs = []
        }
    }

    private func loadPrompts() async {
        isLoadingPrompts = true
        defer { isLoadingPrompts = false }

        do {
            let apiPrompts = try await RalphAPIClient.shared.getPrompts()
            prompts = apiPrompts.map { prompt in
                PromptOption(
                    id: prompt.id,
                    name: prompt.name,
                    path: prompt.path,
                    skills: [] // Skills not in API response
                )
            }
        } catch {
            // Graceful fallback: show empty state when API unavailable
            prompts = []
        }
    }
}

// MARK: - Data Models

struct ConfigOption: Identifiable {
    let id: String
    let name: String
    let description: String
    let hats: [String]
    let isPreset: Bool
}

struct PromptOption: Identifiable {
    let id: String
    let name: String
    let path: String
    let skills: [String]
}

// MARK: - Config Card

private struct ConfigCard: View {
    let config: ConfigOption
    let isSelected: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text(config.name)
                        .font(.headline)
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    Spacer()

                    if config.isPreset {
                        Text("PRESET")
                            .font(.caption2.bold())
                            .foregroundColor(CyberpunkTheme.accentPurple)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(CyberpunkTheme.accentPurple.opacity(0.2))
                            .cornerRadius(4)
                    }

                    if isSelected {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(CyberpunkTheme.accentCyan)
                    }
                }

                Text(config.description)
                    .font(.subheadline)
                    .foregroundColor(CyberpunkTheme.textSecondary)

                // Hat composition
                HStack(spacing: 4) {
                    ForEach(config.hats, id: \.self) { hat in
                        Text(hat)
                            .font(.title3)
                    }
                }
            }
            .padding()
            .background(isSelected ? CyberpunkTheme.accentCyan.opacity(0.1) : CyberpunkTheme.bgTertiary)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(isSelected ? CyberpunkTheme.accentCyan : CyberpunkTheme.border, lineWidth: isSelected ? 2 : 1)
            )
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Prompt Card

private struct PromptCard: View {
    let prompt: PromptOption
    let isSelected: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text(prompt.name)
                        .font(.headline)
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    Spacer()

                    if isSelected {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(CyberpunkTheme.accentCyan)
                    }
                }

                Text(prompt.path)
                    .font(.caption.monospaced())
                    .foregroundColor(CyberpunkTheme.textMuted)

                // Skills
                if !prompt.skills.isEmpty {
                    HStack(spacing: 4) {
                        ForEach(prompt.skills, id: \.self) { skill in
                            Text(skill)
                                .font(.caption2)
                                .foregroundColor(CyberpunkTheme.accentYellow)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(CyberpunkTheme.accentYellow.opacity(0.15))
                                .cornerRadius(4)
                        }
                    }
                }
            }
            .padding()
            .background(isSelected ? CyberpunkTheme.accentCyan.opacity(0.1) : CyberpunkTheme.bgTertiary)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(isSelected ? CyberpunkTheme.accentCyan : CyberpunkTheme.border, lineWidth: isSelected ? 2 : 1)
            )
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Summary Row

private struct SummaryRow: View {
    let icon: String
    let title: String
    let value: String
    let color: Color

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .foregroundColor(color)
                .frame(width: 24)

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)

                Text(value)
                    .font(.subheadline)
                    .foregroundColor(CyberpunkTheme.textPrimary)
                    .lineLimit(1)
            }

            Spacer()
        }
    }
}

#Preview {
    CreateRalphWizard(
        onComplete: { config, prompt, dir in
            #if DEBUG
            print("Creating Ralph: \(config), \(prompt), \(dir)")
            #endif
        },
        onCancel: {
            #if DEBUG
            print("Cancelled")
            #endif
        }
    )
}
