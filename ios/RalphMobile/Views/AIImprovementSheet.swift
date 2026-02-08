import SwiftUI

/// Sheet displaying AI-suggested prompt improvements with accept/reject actions.
struct AIImprovementSheet: View {
    /// The original user prompt
    let originalPrompt: String

    /// The AI-suggested improved prompt
    let suggestedPrompt: String

    /// Called when user accepts the suggestion
    var onAccept: ((String) -> Void)?

    /// Called when user rejects the suggestion
    var onReject: (() -> Void)?

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Header
                    HStack {
                        Image(systemName: "sparkles")
                            .font(.title2)
                            .foregroundColor(CyberpunkTheme.accentPurple)
                        Text("AI Prompt Improvement")
                            .font(.title2)
                            .fontWeight(.bold)
                            .foregroundColor(CyberpunkTheme.textPrimary)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 8)

                    // Original prompt section
                    VStack(alignment: .leading, spacing: 8) {
                        Text("ORIGINAL")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundColor(CyberpunkTheme.accentCyan.opacity(0.7))

                        Text(originalPrompt)
                            .font(.body)
                            .foregroundColor(CyberpunkTheme.textSecondary)
                            .padding(12)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(CyberpunkTheme.bgCard)
                            .cornerRadius(8)
                            .overlay(
                                RoundedRectangle(cornerRadius: 8)
                                    .stroke(CyberpunkTheme.accentCyan.opacity(0.3), lineWidth: 1)
                            )
                    }
                    .accessibilityIdentifier("ai-improvement-original")

                    // Arrow indicator
                    HStack {
                        Spacer()
                        Image(systemName: "arrow.down")
                            .font(.title3)
                            .foregroundColor(CyberpunkTheme.accentPurple)
                        Spacer()
                    }

                    // Suggested prompt section
                    VStack(alignment: .leading, spacing: 8) {
                        Text("SUGGESTED")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundColor(CyberpunkTheme.accentPurple.opacity(0.7))

                        Text(suggestedPrompt)
                            .font(.body)
                            .foregroundColor(CyberpunkTheme.textPrimary)
                            .padding(12)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(CyberpunkTheme.accentPurple.opacity(0.1))
                            .cornerRadius(8)
                            .overlay(
                                RoundedRectangle(cornerRadius: 8)
                                    .stroke(CyberpunkTheme.accentPurple.opacity(0.5), lineWidth: 1)
                            )
                    }
                    .accessibilityIdentifier("ai-improvement-suggested")

                    Spacer(minLength: 20)

                    // Action buttons
                    HStack(spacing: 16) {
                        // Reject button
                        Button {
                            onReject?()
                            dismiss()
                        } label: {
                            HStack {
                                Image(systemName: "xmark")
                                Text("Reject")
                            }
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(CyberpunkTheme.accentRed)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                            .background(CyberpunkTheme.accentRed.opacity(0.15))
                            .cornerRadius(10)
                            .overlay(
                                RoundedRectangle(cornerRadius: 10)
                                    .stroke(CyberpunkTheme.accentRed.opacity(0.5), lineWidth: 1)
                            )
                        }
                        .accessibilityIdentifier("ai-improvement-reject")

                        // Accept button
                        Button {
                            onAccept?(suggestedPrompt)
                            dismiss()
                        } label: {
                            HStack {
                                Image(systemName: "checkmark")
                                Text("Accept")
                            }
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(CyberpunkTheme.accentGreen)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 14)
                            .background(CyberpunkTheme.accentGreen.opacity(0.15))
                            .cornerRadius(10)
                            .overlay(
                                RoundedRectangle(cornerRadius: 10)
                                    .stroke(CyberpunkTheme.accentGreen.opacity(0.5), lineWidth: 1)
                            )
                        }
                        .accessibilityIdentifier("ai-improvement-accept")
                    }
                    .padding(.bottom, 16)
                }
                .padding(.horizontal, 16)
            }
            .background(CyberpunkTheme.bgPrimary)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        onReject?()
                        dismiss()
                    }
                    .foregroundColor(CyberpunkTheme.accentCyan)
                }
            }
            .toolbarBackground(CyberpunkTheme.bgCard, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
        }
        .preferredColorScheme(.dark)
    }
}

/// Loading state view while AI processes the prompt
struct AIImprovementLoadingView: View {
    @State private var isPulsing = false

    var body: some View {
        VStack(spacing: 24) {
            // Animated sparkles
            Image(systemName: "sparkles")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.accentPurple)
                .scaleEffect(isPulsing ? 1.2 : 1.0)
                .opacity(isPulsing ? 0.7 : 1.0)
                .animation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true), value: isPulsing)

            Text("Improving your prompt...")
                .font(.headline)
                .foregroundColor(CyberpunkTheme.textPrimary)

            Text("Claude is analyzing and enhancing your prompt")
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)

            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentPurple))
                .scaleTransform(1.2)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(CyberpunkTheme.bgPrimary)
        .onAppear {
            isPulsing = true
        }
        .accessibilityIdentifier("ai-improvement-loading")
    }
}

/// Error state view when AI improvement fails
struct AIImprovementErrorView: View {
    let error: Error
    var onRetry: (() -> Void)?
    var onDismiss: (() -> Void)?

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 24) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.accentRed)

            Text("Failed to improve prompt")
                .font(.headline)
                .foregroundColor(CyberpunkTheme.textPrimary)

            Text(error.localizedDescription)
                .font(.subheadline)
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            HStack(spacing: 16) {
                Button {
                    onDismiss?()
                    dismiss()
                } label: {
                    Text("Cancel")
                        .font(.system(size: 16, weight: .medium))
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .padding(.horizontal, 24)
                        .padding(.vertical, 12)
                        .background(CyberpunkTheme.bgCard)
                        .cornerRadius(8)
                }

                Button {
                    onRetry?()
                } label: {
                    Text("Retry")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(CyberpunkTheme.accentCyan)
                        .padding(.horizontal, 24)
                        .padding(.vertical, 12)
                        .background(CyberpunkTheme.accentCyan.opacity(0.15))
                        .cornerRadius(8)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(CyberpunkTheme.bgPrimary)
        .accessibilityIdentifier("ai-improvement-error")
    }
}

// MARK: - Scale Transform Modifier

private extension View {
    func scaleTransform(_ scale: CGFloat) -> some View {
        self.scaleEffect(scale)
    }
}

// MARK: - Preview

#Preview("AI Improvement Sheet") {
    AIImprovementSheet(
        originalPrompt: "Fix the bug in login screen please",
        suggestedPrompt: """
        Debug and fix the authentication bug in the login screen.

        **Issue Description:**
        [Describe the specific behavior observed]

        **Expected Behavior:**
        [What should happen when user logs in]

        **Actual Behavior:**
        [What currently happens]

        **Steps to Reproduce:**
        1. Navigate to login screen
        2. Enter credentials
        3. [Describe trigger condition]

        **Environment:**
        - iOS version: [version]
        - Device: [device model]
        """,
        onAccept: { _ in },
        onReject: { }
    )
}

#Preview("Loading State") {
    AIImprovementLoadingView()
}

#Preview("Error State") {
    AIImprovementErrorView(
        error: AnthropicError.apiError("Rate limit exceeded"),
        onRetry: { },
        onDismiss: { }
    )
}
