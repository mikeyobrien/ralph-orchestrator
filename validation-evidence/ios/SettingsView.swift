import SwiftUI

/// Settings view - third screen for navigation validation
struct SettingsView: View {
    @State private var notificationsEnabled = true
    @State private var darkModeEnabled = false
    @State private var soundEnabled = true
    @State private var selectedTheme = "Purple"
    
    let themes = ["Purple", "Teal", "Orange", "Blue"]
    
    var body: some View {
        ZStack {
            // Orange gradient background
            LinearGradient(
                colors: [
                    Color(red: 1.0, green: 0.6, blue: 0.3),  // Orange
                    Color(red: 0.9, green: 0.4, blue: 0.2)   // Darker orange
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            .ignoresSafeArea()
            
            ScrollView {
                VStack(spacing: 24) {
                    // Validation marker
                    Text("Ralph Validation Test")
                        .font(.headline)
                        .foregroundColor(.white.opacity(0.8))
                        .accessibilityIdentifier("settingsValidationMarker")
                    
                    Text("Settings")
                        .font(.largeTitle)
                        .fontWeight(.bold)
                        .foregroundColor(.white)
                    
                    // Settings sections
                    VStack(spacing: 12) {
                        SettingsToggle(title: "Notifications", icon: "bell.fill", isOn: $notificationsEnabled)
                        SettingsToggle(title: "Dark Mode", icon: "moon.fill", isOn: $darkModeEnabled)
                        SettingsToggle(title: "Sound", icon: "speaker.wave.2.fill", isOn: $soundEnabled)
                    }
                    
                    // Theme picker
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Image(systemName: "paintpalette.fill")
                                .foregroundColor(.yellow)
                            Text("Theme")
                                .font(.headline)
                                .foregroundColor(.white)
                        }
                        
                        Picker("Theme", selection: $selectedTheme) {
                            ForEach(themes, id: \.self) { theme in
                                Text(theme).tag(theme)
                            }
                        }
                        .pickerStyle(.segmented)
                        .accessibilityIdentifier("themePicker")
                    }
                    .padding()
                    .background(Color.white.opacity(0.15))
                    .cornerRadius(12)
                    
                    // About section
                    VStack(alignment: .leading, spacing: 8) {
                        Text("About")
                            .font(.headline)
                            .foregroundColor(.white)
                        
                        Text("Ralph Validation App v1.0")
                            .foregroundColor(.white.opacity(0.8))
                        Text("Built to validate iOS SwiftUI apps")
                            .foregroundColor(.white.opacity(0.6))
                        Text("using Ralph Orchestrator's validation feature.")
                            .foregroundColor(.white.opacity(0.6))
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
                    .background(Color.white.opacity(0.15))
                    .cornerRadius(12)
                    
                    Spacer()
                }
                .padding()
            }
        }
        .navigationTitle("Settings")
    }
}

/// Settings toggle component
struct SettingsToggle: View {
    let title: String
    let icon: String
    @Binding var isOn: Bool
    
    var body: some View {
        HStack {
            Image(systemName: icon)
                .foregroundColor(.yellow)
                .frame(width: 30)
            
            Text(title)
                .foregroundColor(.white)
            
            Spacer()
            
            Toggle("", isOn: $isOn)
                .labelsHidden()
                .tint(.green)
        }
        .padding()
        .background(Color.white.opacity(0.15))
        .cornerRadius(12)
    }
}

#Preview {
    NavigationStack {
        SettingsView()
    }
}
