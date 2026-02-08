import SwiftUI

/// Sheet view for browsing and selecting prompt templates
/// Displays bundled templates (read-only) and custom templates (deletable)
struct PromptTemplatesSheet: View {
    @Environment(\.dismiss) private var dismiss
    @ObservedObject var templateStore: TemplateStore

    /// Callback when a template is selected
    var onSelect: (PromptTemplate) -> Void

    /// Current editor content for "Save as Template" feature
    var currentContent: String

    @State private var searchText = ""
    @State private var showingSaveAlert = false
    @State private var newTemplateName = ""

    // MARK: - Filtered Templates

    private var filteredBundled: [PromptTemplate] {
        if searchText.isEmpty {
            return PromptTemplate.bundled
        }
        return PromptTemplate.bundled.filter { template in
            template.name.localizedCaseInsensitiveContains(searchText) ||
            template.description.localizedCaseInsensitiveContains(searchText)
        }
    }

    private var filteredCustom: [PromptTemplate] {
        if searchText.isEmpty {
            return templateStore.customTemplates
        }
        return templateStore.customTemplates.filter { template in
            template.name.localizedCaseInsensitiveContains(searchText) ||
            template.description.localizedCaseInsensitiveContains(searchText)
        }
    }

    // MARK: - Body

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // Search bar
                searchBar
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)

                Divider()
                    .background(CyberpunkTheme.accentCyan.opacity(0.3))

                // Templates list
                ScrollView {
                    LazyVStack(spacing: 0, pinnedViews: [.sectionHeaders]) {
                        // Bundled section
                        bundledSection

                        // Custom section
                        customSection
                    }
                }

                Divider()
                    .background(CyberpunkTheme.accentCyan.opacity(0.3))

                // Save current as template button
                saveCurrentButton
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)
            }
            .background(CyberpunkTheme.bgPrimary)
            .navigationTitle("Prompt Templates")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        dismiss()
                    }
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .fontWeight(.semibold)
                }
            }
            .toolbarBackground(CyberpunkTheme.bgCard, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
        }
        .preferredColorScheme(.dark)
        .alert("Save as Template", isPresented: $showingSaveAlert) {
            TextField("Template name", text: $newTemplateName)
            Button("Cancel", role: .cancel) {
                newTemplateName = ""
            }
            Button("Save") {
                saveCurrentAsTemplate()
            }
            .disabled(newTemplateName.trimmingCharacters(in: .whitespaces).isEmpty)
        } message: {
            Text("Enter a name for your new template")
        }
    }

    // MARK: - Search Bar

    private var searchBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .foregroundColor(CyberpunkTheme.accentCyan.opacity(0.7))
                .font(.system(size: 16))

            TextField("Search templates...", text: $searchText)
                .foregroundColor(.white)
                .font(.system(size: 16))
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(10)
        .accessibilityIdentifier("templates-search")
    }

    // MARK: - Bundled Section

    private var bundledSection: some View {
        Section {
            ForEach(filteredBundled) { template in
                templateRow(template, isBundled: true)
                    .accessibilityIdentifier("template-\(template.id.uuidString)")
            }
        } header: {
            sectionHeader("BUNDLED")
        }
    }

    // MARK: - Custom Section

    private var customSection: some View {
        Section {
            if filteredCustom.isEmpty {
                emptyCustomPlaceholder
            } else {
                ForEach(filteredCustom) { template in
                    templateRow(template, isBundled: false)
                        .accessibilityIdentifier("template-\(template.id.uuidString)")
                }
            }
        } header: {
            sectionHeader("MY TEMPLATES")
        }
    }

    // MARK: - Section Header

    private func sectionHeader(_ title: String) -> some View {
        HStack {
            Text(title)
                .font(.system(size: 12, weight: .semibold))
                .foregroundColor(CyberpunkTheme.accentCyan.opacity(0.7))
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(CyberpunkTheme.bgPrimary)
    }

    // MARK: - Template Row

    private func templateRow(_ template: PromptTemplate, isBundled: Bool) -> some View {
        Button {
            onSelect(template)
            dismiss()
        } label: {
            HStack(spacing: 12) {
                // Icon
                templateIcon(for: template)

                // Text content
                VStack(alignment: .leading, spacing: 4) {
                    Text(template.name)
                        .font(.system(size: 16, weight: .medium))
                        .foregroundColor(.white)

                    Text(template.description)
                        .font(.system(size: 13))
                        .foregroundColor(.gray)
                        .lineLimit(1)
                }

                Spacer()

                // Delete button for custom templates
                if !isBundled {
                    Button(role: .destructive) {
                        deleteTemplate(template)
                    } label: {
                        Image(systemName: "trash")
                            .font(.system(size: 14))
                            .foregroundColor(CyberpunkTheme.accentRed.opacity(0.8))
                            .frame(width: 32, height: 32)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(CyberpunkTheme.bgCard)
            .cornerRadius(10)
        }
        .buttonStyle(.plain)
        .padding(.horizontal, 16)
        .padding(.vertical, 4)
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            if !isBundled {
                Button(role: .destructive) {
                    deleteTemplate(template)
                } label: {
                    Label("Delete", systemImage: "trash")
                }
            }
        }
    }

    // MARK: - Template Icon

    private func templateIcon(for template: PromptTemplate) -> some View {
        let (icon, color): (String, Color) = {
            switch template.name {
            case "Design Start":
                return ("target", CyberpunkTheme.accentMagenta)
            case "Task Start":
                return ("gearshape", CyberpunkTheme.accentCyan)
            case "Code Review":
                return ("magnifyingglass", CyberpunkTheme.accentGreen)
            case "Debug Issue":
                return ("ant", CyberpunkTheme.accentRed)
            default:
                return ("doc.text", CyberpunkTheme.accentCyan)
            }
        }()

        return Image(systemName: icon)
            .font(.system(size: 20))
            .foregroundColor(color)
            .frame(width: 40, height: 40)
            .background(color.opacity(0.15))
            .cornerRadius(8)
    }

    // MARK: - Empty Custom Placeholder

    private var emptyCustomPlaceholder: some View {
        VStack(spacing: 8) {
            Image(systemName: "doc.badge.plus")
                .font(.system(size: 32))
                .foregroundColor(.gray.opacity(0.5))

            Text("No custom templates")
                .font(.system(size: 14))
                .foregroundColor(.gray)

            Text("Save your prompts as templates for quick reuse")
                .font(.system(size: 12))
                .foregroundColor(.gray.opacity(0.7))
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 32)
        .padding(.horizontal, 16)
    }

    // MARK: - Save Current Button

    private var saveCurrentButton: some View {
        Button {
            showingSaveAlert = true
        } label: {
            HStack {
                Image(systemName: "plus.circle.fill")
                    .font(.system(size: 18))
                Text("Save Current as Template")
                    .font(.system(size: 15, weight: .medium))
            }
            .foregroundColor(CyberpunkTheme.accentMagenta)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .background(CyberpunkTheme.accentMagenta.opacity(0.15))
            .cornerRadius(10)
        }
        .disabled(currentContent.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        .opacity(currentContent.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? 0.5 : 1.0)
        .accessibilityIdentifier("templates-save-current")
    }

    // MARK: - Actions

    private func deleteTemplate(_ template: PromptTemplate) {
        templateStore.deleteCustom(id: template.id)
    }

    private func saveCurrentAsTemplate() {
        let trimmedName = newTemplateName.trimmingCharacters(in: .whitespaces)
        guard !trimmedName.isEmpty else { return }

        _ = templateStore.createCustomTemplate(
            name: trimmedName,
            content: currentContent,
            description: "Custom template"
        )

        newTemplateName = ""
    }
}

// MARK: - Preview

#Preview {
    PromptTemplatesSheet(
        templateStore: TemplateStore(),
        onSelect: { _ in },
        currentContent: "# Sample Content\n\nThis is sample content."
    )
}
