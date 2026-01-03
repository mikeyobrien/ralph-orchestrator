import SwiftUI

/// Detail view - second screen for navigation validation
struct DetailView: View {
    @State private var isAnimating = false
    
    var body: some View {
        ZStack {
            // Teal gradient background (different from home)
            LinearGradient(
                colors: [
                    Color(red: 0.2, green: 0.8, blue: 0.8),  // Teal
                    Color(red: 0.1, green: 0.5, blue: 0.7)   // Darker teal
                ],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()
            
            VStack(spacing: 24) {
                // Validation marker
                Text("Ralph Validation Test")
                    .font(.headline)
                    .foregroundColor(.white.opacity(0.8))
                    .accessibilityIdentifier("detailValidationMarker")
                
                Text("Detail Screen")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                    .foregroundColor(.white)
                
                // Animated icon
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 80))
                    .foregroundColor(.white)
                    .scaleEffect(isAnimating ? 1.2 : 1.0)
                    .animation(.easeInOut(duration: 1).repeatForever(autoreverses: true), value: isAnimating)
                    .onAppear {
                        isAnimating = true
                    }
                    .accessibilityIdentifier("detailIcon")
                
                // Feature cards
                VStack(spacing: 12) {
                    FeatureCard(icon: "bolt.fill", title: "Fast", description: "Blazing fast performance")
                    FeatureCard(icon: "lock.fill", title: "Secure", description: "End-to-end encryption")
                    FeatureCard(icon: "globe", title: "Global", description: "Works worldwide")
                }
                
                Spacer()
            }
            .padding()
        }
        .navigationTitle("Details")
    }
}

/// Feature card component
struct FeatureCard: View {
    let icon: String
    let title: String
    let description: String
    
    var body: some View {
        HStack(spacing: 16) {
            Image(systemName: icon)
                .font(.title)
                .foregroundColor(.yellow)
                .frame(width: 40)
            
            VStack(alignment: .leading) {
                Text(title)
                    .font(.headline)
                    .foregroundColor(.white)
                Text(description)
                    .font(.caption)
                    .foregroundColor(.white.opacity(0.7))
            }
            
            Spacer()
        }
        .padding()
        .background(Color.white.opacity(0.15))
        .cornerRadius(12)
    }
}

#Preview {
    NavigationStack {
        DetailView()
    }
}
