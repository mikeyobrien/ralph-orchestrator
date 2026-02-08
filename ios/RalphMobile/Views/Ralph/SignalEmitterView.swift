import SwiftUI

/// Signal emission view based on config's acceptedSignals
/// Users select signal type first, then enter message
struct SignalEmitterView: View {
    let acceptedSignals: [String]
    let onSendSignal: (String, String) -> Void

    @State private var selectedSignal: String? = nil
    @State private var message: String = ""
    @State private var isSending: Bool = false
    @FocusState private var isMessageFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Signal type buttons
            signalTypeButtons

            // Message input (shown when signal selected)
            if selectedSignal != nil {
                messageInputSection
            }
        }
    }

    // MARK: - Signal Type Buttons

    private var signalTypeButtons: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(acceptedSignals, id: \.self) { signal in
                    SignalTypeButton(
                        signal: signal,
                        isSelected: selectedSignal == signal,
                        onTap: {
                            withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                                if selectedSignal == signal {
                                    selectedSignal = nil
                                    message = ""
                                } else {
                                    selectedSignal = signal
                                    isMessageFocused = true
                                }
                            }
                        }
                    )
                }
            }
        }
    }

    // MARK: - Message Input Section

    private var messageInputSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Selected signal indicator
            HStack(spacing: 6) {
                Image(systemName: iconForSignal(selectedSignal ?? ""))
                    .foregroundColor(colorForSignal(selectedSignal ?? ""))

                Text(selectedSignal ?? "")
                    .font(.caption.monospaced())
                    .foregroundColor(colorForSignal(selectedSignal ?? ""))

                Spacer()

                Button {
                    withAnimation {
                        selectedSignal = nil
                        message = ""
                    }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }

            // Message input field
            HStack(spacing: 8) {
                TextField("Enter message...", text: $message, axis: .vertical)
                    .textFieldStyle(.plain)
                    .font(.body)
                    .foregroundColor(CyberpunkTheme.textPrimary)
                    .padding(10)
                    .background(CyberpunkTheme.bgPrimary)
                    .cornerRadius(8)
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(CyberpunkTheme.border, lineWidth: 1)
                    )
                    .focused($isMessageFocused)
                    .lineLimit(1...4)
                    .onSubmit {
                        sendSignal()
                    }

                // Send button
                Button {
                    sendSignal()
                } label: {
                    Group {
                        if isSending {
                            ProgressView()
                                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.bgPrimary))
                                .scaleEffect(0.8)
                        } else {
                            Image(systemName: "arrow.up.circle.fill")
                                .font(.title2)
                        }
                    }
                    .frame(width: 40, height: 40)
                    .background(
                        message.isEmpty
                            ? CyberpunkTheme.textMuted
                            : colorForSignal(selectedSignal ?? "")
                    )
                    .foregroundColor(CyberpunkTheme.bgPrimary)
                    .cornerRadius(8)
                }
                .disabled(message.isEmpty || isSending)
            }

            // Hint text
            Text(hintForSignal(selectedSignal ?? ""))
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .padding(12)
        .background(CyberpunkTheme.bgTertiary)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(colorForSignal(selectedSignal ?? "").opacity(0.3), lineWidth: 1)
        )
        .transition(.opacity.combined(with: .move(edge: .top)))
    }

    // MARK: - Actions

    private func sendSignal() {
        guard let signal = selectedSignal, !message.isEmpty else { return }

        isSending = true

        // Call the callback
        onSendSignal(signal, message)

        // Reset state after brief delay for feedback
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            withAnimation {
                isSending = false
                message = ""
                selectedSignal = nil
            }
        }
    }

    // MARK: - Helpers

    private func iconForSignal(_ signal: String) -> String {
        switch signal {
        case "user.guidance": return "lightbulb.fill"
        case "user.pause": return "pause.circle.fill"
        case "user.priority": return "arrow.up.circle.fill"
        case "user.abort": return "xmark.octagon.fill"
        default: return "paperplane.fill"
        }
    }

    private func colorForSignal(_ signal: String) -> Color {
        switch signal {
        case "user.guidance": return CyberpunkTheme.accentCyan
        case "user.pause": return CyberpunkTheme.accentYellow
        case "user.priority": return CyberpunkTheme.accentPurple
        case "user.abort": return CyberpunkTheme.accentRed
        default: return CyberpunkTheme.accentMagenta
        }
    }

    private func hintForSignal(_ signal: String) -> String {
        switch signal {
        case "user.guidance": return "Provide direction or suggestions to steer the agent"
        case "user.pause": return "Request the agent to pause after current task"
        case "user.priority": return "Change task priority or focus area"
        case "user.abort": return "Request immediate abort of current execution"
        default: return "Send a signal to the running session"
        }
    }
}

// MARK: - Signal Type Button

private struct SignalTypeButton: View {
    let signal: String
    let isSelected: Bool
    let onTap: () -> Void

    private var displayName: String {
        signal.replacingOccurrences(of: "user.", with: "")
    }

    private var icon: String {
        switch signal {
        case "user.guidance": return "lightbulb"
        case "user.pause": return "pause"
        case "user.priority": return "arrow.up"
        case "user.abort": return "xmark"
        default: return "paperplane"
        }
    }

    private var color: Color {
        switch signal {
        case "user.guidance": return CyberpunkTheme.accentCyan
        case "user.pause": return CyberpunkTheme.accentYellow
        case "user.priority": return CyberpunkTheme.accentPurple
        case "user.abort": return CyberpunkTheme.accentRed
        default: return CyberpunkTheme.accentMagenta
        }
    }

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                    .font(.caption)

                Text(displayName)
                    .font(.caption.bold())
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(isSelected ? color.opacity(0.2) : CyberpunkTheme.bgTertiary)
            .foregroundColor(isSelected ? color : CyberpunkTheme.textSecondary)
            .cornerRadius(6)
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(isSelected ? color : CyberpunkTheme.border, lineWidth: isSelected ? 2 : 1)
            )
        }
        .buttonStyle(.plain)
        .shadow(color: isSelected ? color.opacity(0.3) : .clear, radius: 6)
    }
}

#Preview {
    VStack {
        SignalEmitterView(
            acceptedSignals: ["user.guidance", "user.pause", "user.priority", "user.abort"],
            onSendSignal: { type, message in
                #if DEBUG
                print("Signal: \(type), Message: \(message)")
                #endif
            }
        )
    }
    .padding()
    .background(CyberpunkTheme.bgSecondary)
}
