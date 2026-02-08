import SwiftUI

/// View for displaying and browsing available skills
struct SkillsView: View {
    @StateObject private var viewModel = SkillsViewModel()
    @State private var searchText = ""
    @State private var selectedSkill: Skill?

    var filteredSkills: [Skill] {
        if searchText.isEmpty {
            return viewModel.skills
        }
        return viewModel.skills.filter { skill in
            skill.name.localizedCaseInsensitiveContains(searchText) ||
            skill.description.localizedCaseInsensitiveContains(searchText) ||
            skill.tags.contains { $0.localizedCaseInsensitiveContains(searchText) }
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            headerView

            Divider()
                .background(CyberpunkTheme.border)

            // Content
            if viewModel.isLoading {
                loadingView
            } else if let error = viewModel.error {
                errorView(error: error)
            } else if viewModel.skills.isEmpty {
                emptyView
            } else {
                skillsList
            }
        }
        .background(CyberpunkTheme.bgPrimary)
        .task {
            await viewModel.fetchSkills()
        }
        .sheet(item: $selectedSkill) { skill in
            SkillDetailView(skill: skill)
        }
    }

    // MARK: - Header

    private var headerView: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("SKILLS")
                        .font(.system(.headline, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.accentCyan)

                    Text("\(viewModel.skills.count) available")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                }

                Spacer()

                Button {
                    Task {
                        await viewModel.fetchSkills()
                    }
                } label: {
                    Image(systemName: "arrow.clockwise")
                        .font(.body)
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }
            }

            // Search bar
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(CyberpunkTheme.textMuted)

                TextField("Search skills...", text: $searchText)
                    .font(.system(.body, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textPrimary)

                if !searchText.isEmpty {
                    Button {
                        searchText = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(CyberpunkTheme.textMuted)
                    }
                }
            }
            .padding(10)
            .background(CyberpunkTheme.bgTertiary)
            .cornerRadius(8)
        }
        .padding()
    }

    // MARK: - Skills List

    private var skillsList: some View {
        ScrollView {
            LazyVStack(spacing: 8) {
                ForEach(filteredSkills) { skill in
                    SkillRowView(skill: skill) {
                        selectedSkill = skill
                    }
                }
            }
            .padding()
        }
    }

    // MARK: - Loading View

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)

            Text("Loading skills...")
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Error View

    private func errorView(error: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.accentRed)

            Text("Failed to load skills")
                .font(.system(.headline, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textPrimary)

            Text(error)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)

            Button {
                Task {
                    await viewModel.fetchSkills()
                }
            } label: {
                HStack {
                    Image(systemName: "arrow.clockwise")
                    Text("Retry")
                }
                .font(.system(.body, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.bgPrimary)
                .padding(.horizontal, 20)
                .padding(.vertical, 10)
                .background(CyberpunkTheme.accentCyan)
                .cornerRadius(8)
            }
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Empty View

    private var emptyView: some View {
        VStack(spacing: 16) {
            Image(systemName: "cube.transparent")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No skills found")
                .font(.system(.headline, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)

            Text("Skills extend Ralph's capabilities")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Skill Row View

struct SkillRowView: View {
    let skill: Skill
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            VStack(alignment: .leading, spacing: 8) {
                // Header row
                HStack {
                    // Source icon
                    Image(systemName: skill.sourceIcon)
                        .font(.caption)
                        .foregroundColor(skill.isBuiltIn ? CyberpunkTheme.accentCyan : CyberpunkTheme.accentYellow)

                    // Name
                    Text(skill.name)
                        .font(.system(.body, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    Spacer()

                    // Auto-inject badge
                    if skill.autoInject {
                        Text("AUTO")
                            .font(.system(.caption2, design: .monospaced).bold())
                            .foregroundColor(CyberpunkTheme.accentGreen)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(CyberpunkTheme.accentGreen.opacity(0.2))
                            .cornerRadius(4)
                    }

                    Image(systemName: "chevron.right")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }

                // Description
                Text(skill.description)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .lineLimit(2)

                // Tags
                if !skill.tags.isEmpty {
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 6) {
                            ForEach(skill.tags, id: \.self) { tag in
                                Text(tag)
                                    .font(.system(.caption2, design: .monospaced))
                                    .foregroundColor(CyberpunkTheme.textMuted)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(CyberpunkTheme.bgTertiary)
                                    .cornerRadius(4)
                            }
                        }
                    }
                }
            }
            .padding(12)
            .background(CyberpunkTheme.bgSecondary)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(CyberpunkTheme.border, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Skills ViewModel

@MainActor
class SkillsViewModel: ObservableObject {
    @Published var skills: [Skill] = []
    @Published var isLoading = false
    @Published var error: String?

    func fetchSkills() async {
        guard RalphAPIClient.isConfigured else {
            error = "API client not configured"
            return
        }

        isLoading = true
        error = nil

        do {
            skills = try await RalphAPIClient.shared.getSkills()
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }
}

#Preview {
    SkillsView()
        .preferredColorScheme(.dark)
}
