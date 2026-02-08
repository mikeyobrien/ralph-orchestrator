import SwiftUI

/// Config Viewer screen - displays ralph.yml configuration with expandable sections
/// Matches ConfigView from ralph-mobile-ui-documentation.md
struct ConfigView: View {
    @ObservedObject var viewModel: SessionViewModel
    @State private var expandedSections: Set<String> = ["hats"]

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                // Header
                headerSection

                // Config content
                if let content = viewModel.configContent {
                    configContentView(content)
                } else if viewModel.isLoading {
                    loadingView
                } else {
                    emptyConfigView
                }
            }
            .padding()
        }
        .background(CyberpunkTheme.bgPrimary)
        .task {
            await viewModel.fetchConfig()
        }
    }

    // MARK: - Header Section

    private var headerSection: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("CONFIGURATION")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .kerning(2)

                HStack(spacing: 8) {
                    Image(systemName: "gearshape.fill")
                        .foregroundColor(CyberpunkTheme.accentOrange)
                    Text("ralph.yml")
                        .font(.title2.bold())
                        .foregroundColor(CyberpunkTheme.textPrimary)
                }
            }

            Spacer()

            // Refresh button
            Button {
                Task { await viewModel.fetchConfig() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .padding(10)
                    .background(CyberpunkTheme.bgTertiary)
                    .cornerRadius(8)
            }
            .accessibilityIdentifier("config-button-refresh")
        }
    }

    // MARK: - Config Content

    private func configContentView(_ content: String) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            // Parse YAML sections from content
            let sections = parseConfigSections(content)

            ForEach(sections, id: \.name) { section in
                ConfigSectionCard(
                    section: section,
                    isExpanded: expandedSections.contains(section.name),
                    onToggle: {
                        withAnimation(.easeInOut(duration: 0.2)) {
                            if expandedSections.contains(section.name) {
                                expandedSections.remove(section.name)
                            } else {
                                expandedSections.insert(section.name)
                            }
                        }
                    }
                )
                .accessibilityIdentifier("config-section-toggle")
            }

            // Raw YAML view
            rawYamlSection(content)
        }
        .accessibilityIdentifier("config-content-view")
    }

    private func rawYamlSection(_ content: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("RAW YAML")
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .kerning(1)

                Spacer()

                Button {
                    UIPasteboard.general.string = content
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "doc.on.doc")
                        Text("Copy")
                    }
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }
            }

            ScrollView(.horizontal, showsIndicators: false) {
                Text(content)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .padding()
            }
            .background(CyberpunkTheme.bgCard)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(CyberpunkTheme.border, lineWidth: 1)
            )
            .accessibilityIdentifier("config-yaml-display")
        }
    }

    // MARK: - Loading & Empty States

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)

            Text("Loading configuration...")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textSecondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 60)
    }

    private var emptyConfigView: some View {
        VStack(spacing: 12) {
            Image(systemName: "doc.badge.gearshape")
                .font(.system(size: 40))
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No configuration loaded")
                .font(.headline)
                .foregroundColor(CyberpunkTheme.textSecondary)

            Text("Start a session to view its configuration")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)

            Button {
                Task { await viewModel.fetchConfig() }
            } label: {
                Text("Retry")
                    .font(.subheadline.bold())
                    .foregroundColor(CyberpunkTheme.bgPrimary)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 10)
                    .background(CyberpunkTheme.accentCyan)
                    .cornerRadius(8)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(12)
    }

    // MARK: - Config Parsing

    private func parseConfigSections(_ content: String) -> [ConfigSection] {
        // Simple YAML section parsing
        var sections: [ConfigSection] = []
        let lines = content.split(separator: "\n", omittingEmptySubsequences: false)

        var currentSection: String?
        var currentContent: [String] = []

        for line in lines {
            let lineStr = String(line)
            // Top-level keys (no leading whitespace, ends with colon)
            if !lineStr.hasPrefix(" ") && !lineStr.hasPrefix("\t") && lineStr.hasSuffix(":") && !lineStr.contains(" ") {
                // Save previous section
                if let section = currentSection {
                    sections.append(ConfigSection(name: section, content: currentContent.joined(separator: "\n")))
                }
                currentSection = String(lineStr.dropLast())
                currentContent = []
            } else if currentSection != nil {
                currentContent.append(lineStr)
            }
        }

        // Save last section
        if let section = currentSection {
            sections.append(ConfigSection(name: section, content: currentContent.joined(separator: "\n")))
        }

        return sections
    }
}

// MARK: - Config Section Model

struct ConfigSection: Identifiable {
    let name: String
    let content: String

    var id: String { name }

    var icon: String {
        switch name {
        case "cli": return "terminal"
        case "event_loop": return "repeat"
        case "core": return "cpu"
        case "hats": return "theatermasks"
        case "backpressure": return "gauge.with.dots.needle.bottom.50percent"
        default: return "doc.text"
        }
    }

    var accentColor: Color {
        switch name {
        case "cli": return CyberpunkTheme.accentCyan
        case "event_loop": return CyberpunkTheme.accentYellow
        case "core": return CyberpunkTheme.accentMagenta
        case "hats": return CyberpunkTheme.accentOrange
        case "backpressure": return CyberpunkTheme.accentGreen
        default: return CyberpunkTheme.textSecondary
        }
    }
}

// MARK: - Config Section Card

private struct ConfigSectionCard: View {
    let section: ConfigSection
    let isExpanded: Bool
    let onToggle: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            Button(action: onToggle) {
                HStack {
                    Image(systemName: section.icon)
                        .font(.system(size: 14))
                        .foregroundColor(section.accentColor)
                        .frame(width: 24)

                    Text(section.name)
                        .font(.system(.body, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    Spacer()

                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
                .padding()
            }
            .buttonStyle(.plain)

            // Content
            if isExpanded && !section.content.isEmpty {
                Divider()
                    .background(CyberpunkTheme.border)

                Text(section.content)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .padding()
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isExpanded ? section.accentColor.opacity(0.5) : CyberpunkTheme.border, lineWidth: 1)
        )
    }
}

#Preview {
    NavigationStack {
        ConfigView(
            viewModel: SessionViewModel(
                baseURL: URL(string: "http://localhost:8080")!,
                apiKey: ""
            )
        )
    }
}
