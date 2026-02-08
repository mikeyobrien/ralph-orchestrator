import SwiftUI

/// Picker view for selecting a prompt file.
struct PromptPicker: View {
    let prompts: [Prompt]
    @Binding var selection: Prompt?

    var body: some View {
        Picker("Prompt", selection: $selection) {
            Text("None").tag(Prompt?.none)
            ForEach(prompts) { prompt in
                VStack(alignment: .leading) {
                    Text(prompt.name)
                    if !prompt.preview.isEmpty {
                        Text(prompt.preview)
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
                .tag(Prompt?.some(prompt))
            }
        }
    }
}

#Preview {
    PromptPicker(
        prompts: [
            Prompt(path: "prompts/feature.md", name: "feature", preview: "Build a new feature..."),
            Prompt(path: "prompts/bugfix.md", name: "bugfix", preview: "Fix the bug in..."),
        ],
        selection: .constant(nil)
    )
}
