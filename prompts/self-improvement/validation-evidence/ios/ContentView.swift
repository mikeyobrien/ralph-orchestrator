import SwiftUI

/// Main content view with navigation to multiple screens
/// Validates: Purple gradient background, navigation, button interactions
struct ContentView: View {
    @State private var counter = 0
    @State private var showingDetail = false
    
    var body: some View {
        NavigationStack {
            ZStack {
                // Purple gradient background (validation requirement: specific hex color)
                LinearGradient(
                    colors: [
                        Color(red: 0.4, green: 0.494, blue: 0.918),  // #667eea
                        Color(red: 0.463, green: 0.294, blue: 0.635) // #764ba2
                    ],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .ignoresSafeArea()
                
                VStack(spacing: 24) {
                    // Validation marker text (required for validation)
                    Text("Ralph Validation Test")
                        .font(.largeTitle)
                        .fontWeight(.bold)
                        .foregroundColor(.white)
                        .padding()
                        .accessibilityIdentifier("validationMarker")
                    
                    // Counter with button interaction
                    VStack(spacing: 16) {
                        Text("Counter: \(counter)")
                            .font(.title2)
                            .foregroundColor(.white)
                            .accessibilityIdentifier("counterLabel")
                        
                        HStack(spacing: 20) {
                            Button(action: {
                                counter -= 1
                            }) {
                                Image(systemName: "minus.circle.fill")
                                    .font(.system(size: 40))
                                    .foregroundColor(.white)
                            }
                            .accessibilityIdentifier("decrementButton")
                            
                            Button(action: {
                                counter += 1
                            }) {
                                Image(systemName: "plus.circle.fill")
                                    .font(.system(size: 40))
                                    .foregroundColor(.white)
                            }
                            .accessibilityIdentifier("incrementButton")
                        }
                    }
                    .padding()
                    .background(Color.white.opacity(0.2))
                    .cornerRadius(16)
                    
                    // Navigation links (validation requirement: 2-3 screens)
                    VStack(spacing: 12) {
                        NavigationLink(destination: DetailView()) {
                            NavigationButton(title: "Detail Screen", icon: "info.circle")
                        }
                        
                        NavigationLink(destination: SettingsView()) {
                            NavigationButton(title: "Settings Screen", icon: "gear")
                        }
                    }
                    
                    Spacer()
                    
                    // Version info
                    Text("Version 1.0 - Built with Ralph Orchestrator")
                        .font(.caption)
                        .foregroundColor(.white.opacity(0.7))
                }
                .padding()
            }
            .navigationTitle("Home")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button(action: {
                        counter = 0
                    }) {
                        Image(systemName: "arrow.counterclockwise")
                            .foregroundColor(.white)
                    }
                    .accessibilityIdentifier("resetButton")
                }
            }
        }
    }
}

/// Custom navigation button style
struct NavigationButton: View {
    let title: String
    let icon: String
    
    var body: some View {
        HStack {
            Image(systemName: icon)
            Text(title)
            Spacer()
            Image(systemName: "chevron.right")
        }
        .padding()
        .foregroundColor(.white)
        .background(Color.white.opacity(0.2))
        .cornerRadius(12)
    }
}

#Preview {
    ContentView()
}
