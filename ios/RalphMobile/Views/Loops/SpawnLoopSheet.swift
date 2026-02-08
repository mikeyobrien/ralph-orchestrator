import SwiftUI

/// Sheet for spawning a new worktree loop.
struct SpawnLoopSheet: View {
    @State private var prompt: String = ""
    @State private var configPath: String = ""
    @Environment(\.dismiss) private var dismiss

    let onSpawn: (String, String?) -> Void

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("PROMPT")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.textMuted)
                            .kerning(1)
                        TextField("Enter loop prompt...", text: $prompt, axis: .vertical)
                            .font(.system(.body, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.textPrimary)
                            .lineLimit(3...6)
                    }
                } header: {
                    Text("Loop Configuration")
                }

                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("CONFIG PATH (OPTIONAL)")
                            .font(.system(.caption, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.textMuted)
                            .kerning(1)
                        TextField("e.g. presets/feature.yml", text: $configPath)
                            .font(.system(.body, design: .monospaced))
                            .foregroundColor(CyberpunkTheme.textPrimary)
                            .autocapitalization(.none)
                            .disableAutocorrection(true)
                    }
                }
            }
            .scrollContentBackground(.hidden)
            .background(CyberpunkTheme.bgPrimary)
            .navigationTitle("Spawn Loop")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Spawn") {
                        onSpawn(prompt, configPath.isEmpty ? nil : configPath)
                        dismiss()
                    }
                    .font(.body.bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)
                    .disabled(prompt.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
        }
        .preferredColorScheme(.dark)
    }
}

#Preview {
    SpawnLoopSheet { prompt, config in
        print("Spawn: \(prompt), config: \(config ?? "none")")
    }
}
