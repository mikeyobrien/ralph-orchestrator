import SwiftUI

/// Detailed view for a single skill
struct SkillDetailView: View {
    let skill: Skill

    @Environment(\.dismiss) private var dismiss
    @State private var skillContent: String?
    @State private var isLoadingContent = false
    @State private var loadError: String?
    @State private var showContent = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Header
                    headerSection

                    Divider()
                        .background(CyberpunkTheme.border)

                    // Description
                    descriptionSection

                    // Metadata
                    metadataSection

                    // Tags
                    if !skill.tags.isEmpty {
                        tagsSection
                    }

                    // Hats
                    if !skill.hats.isEmpty {
                        hatsSection
                    }

                    // Backends
                    if !skill.backends.isEmpty {
                        backendsSection
                    }

                    // Content section
                    contentSection
                }
                .padding()
            }
            .background(CyberpunkTheme.bgPrimary)
            .navigationTitle(skill.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") {
                        dismiss()
                    }
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }
            }
        }
    }

    // MARK: - Header Section

    private var headerSection: some View {
        HStack(spacing: 12) {
            // Icon
            ZStack {
                RoundedRectangle(cornerRadius: 12)
                    .fill(skill.isBuiltIn ? CyberpunkTheme.accentCyan.opacity(0.2) : CyberpunkTheme.accentYellow.opacity(0.2))
                    .frame(width: 56, height: 56)

                Image(systemName: skill.sourceIcon)
                    .font(.title2)
                    .foregroundColor(skill.isBuiltIn ? CyberpunkTheme.accentCyan : CyberpunkTheme.accentYellow)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(skill.name)
                    .font(.system(.title2, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)

                HStack(spacing: 8) {
                    Text(skill.isBuiltIn ? "Built-in" : "Custom")
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(skill.isBuiltIn ? CyberpunkTheme.accentCyan : CyberpunkTheme.accentYellow)

                    if skill.autoInject {
                        Text("• Auto-inject")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.accentGreen)
                    }
                }
            }

            Spacer()
        }
    }

    // MARK: - Description Section

    private var descriptionSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Description")
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textMuted)

            Text(skill.description)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
        }
    }

    // MARK: - Metadata Section

    private var metadataSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Properties")
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textMuted)

            HStack(spacing: 16) {
                MetadataItem(
                    icon: "cube.fill",
                    label: "Source",
                    value: skill.source
                )

                MetadataItem(
                    icon: "bolt.fill",
                    label: "Auto-inject",
                    value: skill.autoInject ? "Yes" : "No"
                )
            }
        }
    }

    // MARK: - Tags Section

    private var tagsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Tags")
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textMuted)

            FlowLayout(spacing: 8) {
                ForEach(skill.tags, id: \.self) { tag in
                    Text(tag)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textPrimary)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 6)
                        .background(CyberpunkTheme.bgTertiary)
                        .cornerRadius(6)
                }
            }
        }
    }

    // MARK: - Hats Section

    private var hatsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Available for Hats")
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textMuted)

            FlowLayout(spacing: 8) {
                ForEach(skill.hats, id: \.self) { hat in
                    HStack(spacing: 4) {
                        Image(systemName: "party.popper")
                            .font(.caption2)
                        Text(hat)
                    }
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(CyberpunkTheme.accentCyan.opacity(0.15))
                    .cornerRadius(6)
                }
            }
        }
    }

    // MARK: - Backends Section

    private var backendsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Supported Backends")
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.textMuted)

            FlowLayout(spacing: 8) {
                ForEach(skill.backends, id: \.self) { backend in
                    HStack(spacing: 4) {
                        Image(systemName: "server.rack")
                            .font(.caption2)
                        Text(backend)
                    }
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentYellow)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(CyberpunkTheme.accentYellow.opacity(0.15))
                    .cornerRadius(6)
                }
            }
        }
    }

    // MARK: - Content Section

    private var contentSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Content")
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.textMuted)

                Spacer()

                if !showContent {
                    Button {
                        Task {
                            await loadSkillContent()
                        }
                    } label: {
                        HStack(spacing: 4) {
                            if isLoadingContent {
                                ProgressView()
                                    .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                                    .scaleEffect(0.7)
                            } else {
                                Image(systemName: "arrow.down.doc")
                            }
                            Text("Load")
                        }
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.accentCyan)
                    }
                    .disabled(isLoadingContent)
                }
            }

            if let error = loadError {
                Text(error)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentRed)
            } else if showContent, let content = skillContent {
                ScrollView(.horizontal, showsIndicators: true) {
                    Text(content)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textSecondary)
                        .padding()
                        .background(CyberpunkTheme.bgTertiary)
                        .cornerRadius(8)
                }
                .frame(maxHeight: 300)
            }
        }
    }

    // MARK: - Actions

    private func loadSkillContent() async {
        guard RalphAPIClient.isConfigured else {
            loadError = "API client not configured"
            return
        }

        isLoadingContent = true
        loadError = nil

        do {
            let response = try await RalphAPIClient.shared.loadSkill(name: skill.name)
            skillContent = response.content
            showContent = true
        } catch {
            loadError = error.localizedDescription
        }

        isLoadingContent = false
    }
}

// MARK: - Metadata Item

private struct MetadataItem: View {
    let icon: String
    let label: String
    let value: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)

            VStack(alignment: .leading, spacing: 2) {
                Text(label)
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)

                Text(value)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textPrimary)
            }
        }
        .padding(10)
        .background(CyberpunkTheme.bgSecondary)
        .cornerRadius(8)
    }
}

// FlowLayout is defined in HatFlowView.swift

#Preview {
    SkillDetailView(skill: Skill(
        name: "ralph-tools",
        description: "Core tools for Ralph orchestration including task management and event emission.",
        tags: ["core", "tools", "orchestration"],
        hats: ["builder", "reviewer", "fixer"],
        backends: ["claude", "gemini"],
        autoInject: true,
        source: "built-in"
    ))
    .preferredColorScheme(.dark)
}
