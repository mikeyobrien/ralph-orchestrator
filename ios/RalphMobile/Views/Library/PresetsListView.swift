import SwiftUI

/// Browser for available configuration presets.
/// Fetches from GET /api/presets and supports search + tap-to-view YAML content.
struct PresetsListView: View {
    @State private var presets: [PresetItem] = []
    @State private var isLoading: Bool = false
    @State private var errorMessage: String?
    @State private var searchText: String = ""
    @State private var selectedPreset: PresetItem?
    @State private var presetContent: String = ""
    @State private var showContentSheet: Bool = false

    private var filteredPresets: [PresetItem] {
        if searchText.isEmpty {
            return presets
        }
        return presets.filter { $0.name.localizedCaseInsensitiveContains(searchText) }
    }

    var body: some View {
        Group {
            if isLoading {
                loadingView
            } else if let error = errorMessage {
                errorView(error)
            } else if presets.isEmpty {
                emptyView
            } else {
                presetsContent
            }
        }
        .task {
            await loadPresets()
        }
        .sheet(isPresented: $showContentSheet) {
            presetContentSheet
        }
    }

    // MARK: - States

    private var loadingView: some View {
        VStack(spacing: 12) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
            Text("Loading presets...")
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
                Task { await loadPresets() }
            }
            .font(.caption.bold())
            .foregroundColor(CyberpunkTheme.accentCyan)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    private var emptyView: some View {
        VStack(spacing: 12) {
            Image(systemName: "slider.horizontal.3")
                .font(.title)
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("No presets found")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("Place preset YAML files in presets/")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted.opacity(0.7))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }

    // MARK: - Content

    private var presetsContent: some View {
        VStack(spacing: 0) {
            // Search bar
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)

                TextField("Search presets...", text: $searchText)
                    .font(.system(.subheadline, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textPrimary)

                if !searchText.isEmpty {
                    Button {
                        searchText = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .font(.caption)
                            .foregroundColor(CyberpunkTheme.textMuted)
                    }
                }
            }
            .padding(10)
            .background(CyberpunkTheme.bgCard)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(CyberpunkTheme.border, lineWidth: 1)
            )
            .padding(.horizontal)
            .padding(.top, 8)

            // List
            ScrollView {
                LazyVStack(spacing: 8) {
                    ForEach(filteredPresets) { preset in
                        PresetCard(preset: preset)
                            .onTapGesture {
                                selectedPreset = preset
                                Task { await loadPresetContent(preset) }
                            }
                    }
                }
                .padding()
            }
            .accessibilityIdentifier("presets-list-view")
        }
    }

    // MARK: - Content Sheet

    private var presetContentSheet: some View {
        NavigationStack {
            ScrollView {
                Text(presetContent)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
            }
            .background(CyberpunkTheme.bgPrimary)
            .navigationTitle(selectedPreset?.name ?? "Preset")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") {
                        showContentSheet = false
                    }
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }

                ToolbarItem(placement: .topBarLeading) {
                    Button {
                        UIPasteboard.general.string = presetContent
                    } label: {
                        Image(systemName: "doc.on.doc")
                            .foregroundColor(CyberpunkTheme.accentCyan)
                    }
                }
            }
        }
    }

    // MARK: - Data

    private func loadPresets() async {
        guard RalphAPIClient.isConfigured else {
            errorMessage = "API client not configured"
            return
        }

        isLoading = true
        errorMessage = nil

        do {
            presets = try await RalphAPIClient.shared.getPresets()
            isLoading = false
        } catch {
            isLoading = false
            errorMessage = error.localizedDescription
        }
    }

    private func loadPresetContent(_ preset: PresetItem) async {
        do {
            let response = try await RalphAPIClient.shared.getConfigContent(path: preset.path)
            presetContent = response.content
            showContentSheet = true
        } catch {
            presetContent = "Failed to load content: \(error.localizedDescription)"
            showContentSheet = true
        }
    }
}

// MARK: - Preset Card

private struct PresetCard: View {
    let preset: PresetItem

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(preset.name)
                    .font(.system(.subheadline, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)

                Spacer()

                Image(systemName: "chevron.right")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            if !preset.description.isEmpty {
                Text(preset.description)
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .lineLimit(2)
            }

            Text(preset.path)
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
        .accessibilityIdentifier("presets-item-\(preset.name)")
    }
}
