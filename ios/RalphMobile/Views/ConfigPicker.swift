import SwiftUI

/// Picker view for selecting a configuration preset.
struct ConfigPicker: View {
    let configs: [Config]
    @Binding var selection: Config?

    var body: some View {
        Picker("Config", selection: $selection) {
            Text("None").tag(Config?.none)
            ForEach(configs) { config in
                VStack(alignment: .leading) {
                    Text(config.name)
                    if !config.description.isEmpty {
                        Text(config.description)
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
                .tag(Config?.some(config))
            }
        }
    }
}

#Preview {
    ConfigPicker(
        configs: [
            Config(path: "presets/feature.yml", name: "feature", description: "Feature Development"),
            Config(path: "presets/debug.yml", name: "debug", description: "Debug Mode"),
        ],
        selection: .constant(nil)
    )
}
