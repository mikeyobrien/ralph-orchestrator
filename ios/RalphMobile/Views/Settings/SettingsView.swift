import SwiftUI
import UniformTypeIdentifiers

/// Notification posted when server credentials change and need to be reloaded
extension Notification.Name {
    static let serverCredentialsDidChange = Notification.Name("serverCredentialsDidChange")
}

/// Settings view with sections for Backend, Defaults, Notifications, Appearance
/// Based on V3 architecture specification
struct AppSettingsView: View {
    // MARK: - Server Settings
    @AppStorage("serverURL") private var serverURL: String = "http://127.0.0.1:8080"

    // MARK: - Backend Settings
    @AppStorage("defaultBackend") private var defaultBackend: String = "Claude"
    @AppStorage("apiHost") private var apiHost: String = "api.anthropic.com"

    // MARK: - Default Settings
    @AppStorage("maxIterations") private var maxIterations: Int = 100
    @AppStorage("idleTimeout") private var idleTimeout: String = "30 min"
    @AppStorage("defaultConfig") private var defaultConfig: String = "tdd-red-green"

    // MARK: - Notification Settings
    @AppStorage("completionAlerts") private var completionAlerts: Bool = true
    @AppStorage("errorAlerts") private var errorAlerts: Bool = true
    @AppStorage("soundEnabled") private var soundEnabled: Bool = false

    // MARK: - Appearance Settings
    @AppStorage("selectedTheme") private var selectedTheme: String = "Cyberpunk"
    @AppStorage("compactMode") private var compactMode: Bool = false

    // MARK: - Config Export/Import State
    @State private var isExporting: Bool = false
    @State private var isImporting: Bool = false
    @State private var showFileImporter: Bool = false
    @State private var showImportConfirmation: Bool = false
    @State private var pendingImportContent: String = ""
    @State private var configOperationMessage: String?
    @State private var showShareSheet: Bool = false
    @State private var exportedContent: String = ""
    @State private var exportedFilename: String = "ralph-config.yml"

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                // Header
                settingsHeader

                // Server section (ralph-mobile-server)
                SettingsSection(title: "Server") {
                    EmptyView().accessibilityIdentifier("settings-section-server")
                    SettingsTextRow(
                        title: "Server URL",
                        text: $serverURL,
                        placeholder: "http://127.0.0.1:8080"
                    )
                    .accessibilityIdentifier("settings-field-server-url")
                }

                // Configuration Export/Import section
                SettingsSection(title: "Configuration") {
                    EmptyView().accessibilityIdentifier("settings-section-configuration")

                    // Export button
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Export Config")
                                .font(.subheadline)
                                .foregroundColor(CyberpunkTheme.textPrimary)
                            Text("Download current ralph.yml")
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.textMuted)
                        }

                        Spacer()

                        if isExporting {
                            ProgressView()
                                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                        } else {
                            Button {
                                Task { await exportConfig() }
                            } label: {
                                Image(systemName: "square.and.arrow.up")
                                    .font(.title3)
                                    .foregroundColor(CyberpunkTheme.accentCyan)
                            }
                        }
                    }
                    .padding(12)
                    .overlay(
                        Rectangle()
                            .fill(CyberpunkTheme.border)
                            .frame(height: 1),
                        alignment: .bottom
                    )
                    .accessibilityIdentifier("settings-button-export-config")

                    // Import button
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Import Config")
                                .font(.subheadline)
                                .foregroundColor(CyberpunkTheme.textPrimary)
                            Text("Load a YAML config file")
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.textMuted)
                        }

                        Spacer()

                        if isImporting {
                            ProgressView()
                                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                        } else {
                            Button {
                                showFileImporter = true
                            } label: {
                                Image(systemName: "square.and.arrow.down")
                                    .font(.title3)
                                    .foregroundColor(CyberpunkTheme.accentPurple)
                            }
                        }
                    }
                    .padding(12)
                    .accessibilityIdentifier("settings-button-import-config")

                    // Status message
                    if let message = configOperationMessage {
                        HStack {
                            Image(systemName: message.contains("Error") ? "xmark.circle" : "checkmark.circle")
                                .foregroundColor(message.contains("Error") ? CyberpunkTheme.accentRed : CyberpunkTheme.accentGreen)
                            Text(message)
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.textSecondary)
                            Spacer()
                        }
                        .padding(.horizontal, 12)
                        .padding(.bottom, 8)
                    }
                }

                // Backend section
                SettingsSection(title: "Backend") {
                    EmptyView().accessibilityIdentifier("settings-section-backend")
                    SettingsPickerRow(
                        title: "Default Backend",
                        selection: $defaultBackend,
                        options: ["Claude", "Kiro", "Custom"]
                    )
                    .accessibilityIdentifier("settings-picker-default-backend")

                    SettingsTextRow(
                        title: "API Host",
                        text: $apiHost,
                        placeholder: "api.anthropic.com"
                    )
                    .accessibilityIdentifier("settings-field-api-host")
                }

                // Defaults section
                SettingsSection(title: "Defaults") {
                    EmptyView().accessibilityIdentifier("settings-section-defaults")
                    SettingsStepperRow(
                        title: "Max Iterations",
                        value: $maxIterations,
                        range: 10...500,
                        step: 10
                    )
                    .accessibilityIdentifier("settings-stepper-max-iterations")

                    SettingsPickerRow(
                        title: "Idle Timeout",
                        selection: $idleTimeout,
                        options: ["5 min", "15 min", "30 min", "1 hour", "Never"]
                    )
                    .accessibilityIdentifier("settings-picker-idle-timeout")

                    SettingsPickerRow(
                        title: "Default Config",
                        selection: $defaultConfig,
                        options: ["tdd-red-green", "spec-driven", "feature", "debug"]
                    )
                    .accessibilityIdentifier("settings-picker-default-config")
                }

                // Notifications section
                SettingsSection(title: "Notifications (Coming Soon)") {
                    EmptyView().accessibilityIdentifier("settings-section-notifications")
                    SettingsToggleRow(
                        title: "Completion Alerts",
                        isOn: $completionAlerts,
                        description: "Notify when Ralph completes"
                    )
                    .disabled(true)
                    .opacity(0.5)
                    .accessibilityIdentifier("settings-toggle-completion-alerts")

                    SettingsToggleRow(
                        title: "Error Alerts",
                        isOn: $errorAlerts,
                        description: "Notify on errors"
                    )
                    .disabled(true)
                    .opacity(0.5)
                    .accessibilityIdentifier("settings-toggle-error-alerts")

                    SettingsToggleRow(
                        title: "Sound",
                        isOn: $soundEnabled,
                        description: "Play notification sounds"
                    )
                    .disabled(true)
                    .opacity(0.5)
                    .accessibilityIdentifier("settings-toggle-sound")
                }

                // Appearance section
                SettingsSection(title: "Appearance (Coming Soon)") {
                    EmptyView().accessibilityIdentifier("settings-section-appearance")
                    SettingsPickerRow(
                        title: "Theme",
                        selection: $selectedTheme,
                        options: ["Cyberpunk", "Dark", "Light", "System"]
                    )
                    .disabled(true)
                    .opacity(0.5)
                    .accessibilityIdentifier("settings-picker-theme")

                    SettingsToggleRow(
                        title: "Compact Mode",
                        isOn: $compactMode,
                        description: "Reduce spacing and padding"
                    )
                    .disabled(true)
                    .opacity(0.5)
                    .accessibilityIdentifier("settings-toggle-compact-mode")
                }

                // Version info
                versionFooter
            }
            .padding()
        }
        .background(CyberpunkTheme.bgPrimary)
        .onChange(of: serverURL) { _ in
            // Notify ContentView to reconfigure API client when URL changes
            NotificationCenter.default.post(name: .serverCredentialsDidChange, object: nil)
        }
        .onDisappear {
            UserDefaults.standard.synchronize()
        }
        .sheet(isPresented: $showShareSheet) {
            ActivityViewController(activityItems: [exportedContent])
        }
        .fileImporter(
            isPresented: $showFileImporter,
            allowedContentTypes: [.yaml, .plainText],
            allowsMultipleSelection: false
        ) { result in
            handleFileImport(result)
        }
        .confirmationDialog(
            "Import Configuration?",
            isPresented: $showImportConfirmation,
            titleVisibility: .visible
        ) {
            Button("Import", role: .destructive) {
                Task { await importConfig() }
            }
            Button("Cancel", role: .cancel) { }
        } message: {
            Text("This will overwrite the current ralph.yml configuration on the server.")
        }
    }

    // MARK: - Header

    private var settingsHeader: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("Settings")
                    .font(.title2.bold())
                    .foregroundColor(CyberpunkTheme.textPrimary)

                Text("Configure Ralph behavior")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            Spacer()
        }
    }

    // MARK: - Version Footer

    private var versionFooter: some View {
        VStack(spacing: 4) {
            Text("Ralph Orchestrator")
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("v\(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?.?.?")")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .padding(.top, 20)
    }

    // MARK: - Config Export/Import

    private func exportConfig() async {
        guard RalphAPIClient.isConfigured else {
            configOperationMessage = "Error: API client not configured"
            return
        }

        isExporting = true
        configOperationMessage = nil

        do {
            let response = try await RalphAPIClient.shared.exportConfig()
            exportedContent = response.content
            exportedFilename = response.filename
            isExporting = false
            showShareSheet = true
        } catch {
            isExporting = false
            configOperationMessage = "Error: \(error.localizedDescription)"
        }
    }

    private func importConfig() async {
        guard RalphAPIClient.isConfigured else {
            configOperationMessage = "Error: API client not configured"
            return
        }

        isImporting = true
        configOperationMessage = nil

        do {
            let response = try await RalphAPIClient.shared.importConfig(content: pendingImportContent)
            isImporting = false
            configOperationMessage = "Imported: \(response.path)"
            pendingImportContent = ""
        } catch {
            isImporting = false
            configOperationMessage = "Error: \(error.localizedDescription)"
        }
    }

    private func handleFileImport(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            guard let url = urls.first else { return }
            guard url.startAccessingSecurityScopedResource() else {
                configOperationMessage = "Error: Cannot access file"
                return
            }
            defer { url.stopAccessingSecurityScopedResource() }

            do {
                let content = try String(contentsOf: url, encoding: .utf8)
                pendingImportContent = content
                showImportConfirmation = true
            } catch {
                configOperationMessage = "Error: \(error.localizedDescription)"
            }

        case .failure(let error):
            configOperationMessage = "Error: \(error.localizedDescription)"
        }
    }
}

// MARK: - Settings Section

struct SettingsSection<Content: View>: View {
    let title: String
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
                .textCase(.uppercase)
                .padding(.leading, 4)

            VStack(spacing: 0) {
                content
            }
            .background(CyberpunkTheme.bgCard)
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(CyberpunkTheme.border, lineWidth: 1)
            )
        }
    }
}

// MARK: - Settings Toggle Row

struct SettingsToggleRow: View {
    let title: String
    @Binding var isOn: Bool
    var description: String? = nil

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.subheadline)
                    .foregroundColor(CyberpunkTheme.textPrimary)

                if let description = description {
                    Text(description)
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }

            Spacer()

            Toggle("", isOn: $isOn)
                .toggleStyle(CyberpunkToggleStyle())
        }
        .padding(12)
        .overlay(
            Rectangle()
                .fill(CyberpunkTheme.border)
                .frame(height: 1),
            alignment: .bottom
        )
    }
}

// MARK: - Settings Picker Row

struct SettingsPickerRow: View {
    let title: String
    @Binding var selection: String
    let options: [String]

    var body: some View {
        HStack {
            Text(title)
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textPrimary)

            Spacer()

            Menu {
                ForEach(options, id: \.self) { option in
                    Button(option) {
                        selection = option
                    }
                }
            } label: {
                HStack(spacing: 4) {
                    Text(selection)
                        .font(.system(.subheadline, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textSecondary)

                    Image(systemName: "chevron.down")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }
        }
        .padding(12)
        .overlay(
            Rectangle()
                .fill(CyberpunkTheme.border)
                .frame(height: 1),
            alignment: .bottom
        )
    }
}

// MARK: - Settings Text Row

struct SettingsTextRow: View {
    let title: String
    @Binding var text: String
    let placeholder: String
    var onCommit: (() -> Void)? = nil

    var body: some View {
        HStack {
            Text(title)
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textPrimary)

            Spacer()

            TextField(placeholder, text: $text)
                .font(.system(.subheadline, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
                .multilineTextAlignment(.trailing)
                .frame(maxWidth: 180)
                .onSubmit {
                    onCommit?()
                }
        }
        .padding(12)
        .overlay(
            Rectangle()
                .fill(CyberpunkTheme.border)
                .frame(height: 1),
            alignment: .bottom
        )
    }
}

// MARK: - Settings Secure Row

struct SettingsSecureRow: View {
    let title: String
    @Binding var text: String
    let placeholder: String
    var onCommit: ((String) -> Void)? = nil
    @State private var isRevealed: Bool = false

    var body: some View {
        HStack {
            Text(title)
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textPrimary)

            Spacer()

            HStack(spacing: 8) {
                if isRevealed {
                    TextField(placeholder, text: $text)
                        .font(.system(.subheadline, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textSecondary)
                        .multilineTextAlignment(.trailing)
                        .frame(maxWidth: 150)
                        .onSubmit {
                            onCommit?(text)
                        }
                } else {
                    Text(text.isEmpty ? placeholder : "••••••••")
                        .font(.system(.subheadline, design: .monospaced))
                        .foregroundColor(text.isEmpty ? CyberpunkTheme.textMuted : CyberpunkTheme.textSecondary)
                }

                Button {
                    // When hiding, save the value if there's a callback
                    if isRevealed {
                        onCommit?(text)
                    }
                    isRevealed.toggle()
                } label: {
                    Image(systemName: isRevealed ? "eye.slash" : "eye")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }
        }
        .padding(12)
        .overlay(
            Rectangle()
                .fill(CyberpunkTheme.border)
                .frame(height: 1),
            alignment: .bottom
        )
    }
}

// MARK: - Settings Stepper Row

struct SettingsStepperRow: View {
    let title: String
    @Binding var value: Int
    let range: ClosedRange<Int>
    let step: Int

    var body: some View {
        HStack {
            Text(title)
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textPrimary)

            Spacer()

            HStack(spacing: 12) {
                Button {
                    if value - step >= range.lowerBound {
                        value -= step
                    }
                } label: {
                    Image(systemName: "minus.circle")
                        .font(.title3)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }

                Text("\(value)")
                    .font(.system(.subheadline, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textSecondary)
                    .frame(minWidth: 40)

                Button {
                    if value + step <= range.upperBound {
                        value += step
                    }
                } label: {
                    Image(systemName: "plus.circle")
                        .font(.title3)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }
        }
        .padding(12)
        .overlay(
            Rectangle()
                .fill(CyberpunkTheme.border)
                .frame(height: 1),
            alignment: .bottom
        )
    }
}

// MARK: - Activity View Controller (Share Sheet)

struct ActivityViewController: UIViewControllerRepresentable {
    let activityItems: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

#Preview {
    AppSettingsView()
}
