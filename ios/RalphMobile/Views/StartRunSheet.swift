import SwiftUI

/// Sheet view for starting a new Ralph run with config and prompt selection.
struct StartRunSheet: View {
    @ObservedObject var viewModel: SessionViewModel
    @Binding var isPresented: Bool

    @State private var selectedConfig: Config?
    @State private var selectedPrompt: Prompt?

    var body: some View {
        NavigationStack {
            Form {
                Section("Configuration") {
                    ConfigPicker(configs: viewModel.configs, selection: $selectedConfig)
                        .accessibilityIdentifier("start-run-picker-config")
                }

                Section("Prompt") {
                    PromptPicker(prompts: viewModel.prompts, selection: $selectedPrompt)
                        .accessibilityIdentifier("start-run-picker-prompt")
                }

                if let error = viewModel.startRunError {
                    Section {
                        Label(error, systemImage: "exclamationmark.triangle")
                            .foregroundColor(.red)
                    }
                }
            }
            .navigationTitle("Start Run")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        isPresented = false
                    }
                    .accessibilityIdentifier("start-run-button-cancel")
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Start") {
                        guard let config = selectedConfig, let prompt = selectedPrompt else { return }
                        Task {
                            await viewModel.startRun(config: config, prompt: prompt)
                            if viewModel.startRunError == nil {
                                isPresented = false
                            }
                        }
                    }
                    .disabled(selectedConfig == nil || selectedPrompt == nil || viewModel.isStartingRun)
                    .accessibilityIdentifier("start-run-button-start")
                }
            }
            .overlay {
                if viewModel.isStartingRun {
                    ProgressView("Starting...")
                        .padding()
                        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
                }
            }
            .task {
                await viewModel.loadConfigs()
                await viewModel.loadPrompts()
            }
        }
    }
}

#Preview {
    StartRunSheet(
        viewModel: SessionViewModel(
            baseURL: URL(string: "http://127.0.0.1:8080")!,
            apiKey: "test"
        ),
        isPresented: .constant(true)
    )
}
