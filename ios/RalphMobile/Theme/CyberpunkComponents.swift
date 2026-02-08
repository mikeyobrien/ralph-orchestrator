import SwiftUI

// MARK: - Status Indicator (Pulsing Dot)

/// Animated status indicator with pulsing effect
/// Matches the StatusIndicator from ralph-mobile-prototype.jsx
struct StatusIndicator: View {
    let status: ExecutionStatus
    @State private var isPulsing = false

    enum ExecutionStatus: String {
        case running
        case completed
        case pending
        case paused
        case error
        case idle

        var color: Color {
            switch self {
            case .running: return CyberpunkTheme.accentCyan
            case .completed: return CyberpunkTheme.accentGreen
            case .pending: return CyberpunkTheme.accentYellow
            case .paused: return CyberpunkTheme.accentOrange
            case .error: return CyberpunkTheme.accentRed
            case .idle: return CyberpunkTheme.textMuted
            }
        }

        var shouldPulse: Bool {
            self == .running
        }
    }

    var body: some View {
        Circle()
            .fill(status.color)
            .frame(width: 10, height: 10)
            .shadow(color: status.color.opacity(isPulsing ? 0.8 : 0.3), radius: isPulsing ? 8 : 4)
            .animation(
                status.shouldPulse
                    ? .easeInOut(duration: 1).repeatForever(autoreverses: true)
                    : .default,
                value: isPulsing
            )
            .onAppear {
                if status.shouldPulse {
                    isPulsing = true
                }
            }
            .onChange(of: status) { newStatus in
                isPulsing = newStatus.shouldPulse
            }
    }
}

// MARK: - Glow Border

/// Container with animated glowing border
/// Matches GlowBorder from ralph-mobile-prototype.jsx
struct GlowBorder<Content: View>: View {
    let color: Color
    var intensity: Double = 0.5
    var cornerRadius: CGFloat = 8
    var animated: Bool = false
    @ViewBuilder let content: () -> Content

    @State private var glowIntensity: Double = 0.3

    var body: some View {
        content()
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius)
                    .stroke(color, lineWidth: 1.5)
            )
            .shadow(color: color.opacity(glowIntensity), radius: 10)
            .shadow(color: color.opacity(glowIntensity * 0.5), radius: 20)
            .animation(
                animated
                    ? .easeInOut(duration: 2).repeatForever(autoreverses: true)
                    : .default,
                value: glowIntensity
            )
            .onAppear {
                if animated {
                    glowIntensity = intensity
                }
            }
    }
}

// MARK: - Hat Status Card

/// Displays current hat information with icon and status
/// Matches HatStatusCard from ralph-mobile-prototype.jsx
struct HatStatusCard: View {
    let hatName: String
    let iteration: Int
    let triggerEvent: String?
    let publishedEvents: [String]

    private var hatEmoji: String {
        switch hatName.lowercased() {
        case "planner", "📋 planner": return "📋"
        case "builder", "🔨 builder", "⚙️ builder": return "⚙️"
        case "reviewer", "👁️ reviewer": return "👁️"
        case "design critic", "⚖️ design critic": return "⚖️"
        case "tester", "✅ validator": return "✅"
        case "inquisitor", "🎯 inquisitor": return "🎯"
        case "architect", "💭 architect": return "💭"
        case "explorer", "🔍 explorer": return "🔍"
        case "committer", "📦 committer": return "📦"
        case "task writer", "📝 task writer": return "📝"
        case "ralph": return "🎭"
        default: return "🤖"
        }
    }

    private var displayName: String {
        // Remove emoji from name if present
        hatName
            .replacingOccurrences(of: "📋 ", with: "")
            .replacingOccurrences(of: "⚙️ ", with: "")
            .replacingOccurrences(of: "👁️ ", with: "")
            .replacingOccurrences(of: "⚖️ ", with: "")
            .replacingOccurrences(of: "✅ ", with: "")
            .replacingOccurrences(of: "🎯 ", with: "")
            .replacingOccurrences(of: "💭 ", with: "")
            .replacingOccurrences(of: "🔍 ", with: "")
            .replacingOccurrences(of: "📦 ", with: "")
            .replacingOccurrences(of: "📝 ", with: "")
            .replacingOccurrences(of: "🔨 ", with: "")
    }

    private var hatColor: Color {
        CyberpunkTheme.hatColor(for: hatName)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header with hat icon and name
            HStack(spacing: 12) {
                Text(hatEmoji)
                    .font(.system(size: 32))

                VStack(alignment: .leading, spacing: 2) {
                    Text(displayName.capitalized)
                        .font(.headline)
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    Text("Iteration \(iteration)")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }

                Spacer()

                StatusIndicator(status: .running)
            }

            Divider()
                .background(CyberpunkTheme.border)

            // Trigger event
            if let trigger = triggerEvent {
                HStack(spacing: 8) {
                    Image(systemName: "bolt.fill")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.accentYellow)

                    Text("Triggered by:")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textSecondary)

                    Text(trigger)
                        .font(.caption.monospaced())
                        .foregroundColor(CyberpunkTheme.accentCyan)
                }
            }

            // Published events
            if !publishedEvents.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.accentGreen)

                    Text("Publishes:")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textSecondary)

                    ForEach(publishedEvents, id: \.self) { event in
                        Text(event)
                            .font(.caption.monospaced())
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(CyberpunkTheme.bgTertiary)
                            .cornerRadius(4)
                            .foregroundColor(CyberpunkTheme.accentGreen)
                    }
                }
            }
        }
        .padding()
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(hatColor.opacity(0.5), lineWidth: 1)
        )
        .shadow(color: hatColor.opacity(0.2), radius: 10)
    }
}

// MARK: - Tool Call Card

/// Expandable card showing tool call details
/// Matches ToolCallCard from ralph-mobile-prototype.jsx
struct ToolCallCard: View {
    let toolName: String
    let status: StatusIndicator.ExecutionStatus
    let input: [String: Any]?
    let output: String?
    let duration: Int? // milliseconds
    let timestamp: Date

    @State private var isExpanded = false

    private var toolColor: Color {
        switch toolName.lowercased() {
        case "bash": return CyberpunkTheme.toolBash
        case "read_file", "read": return CyberpunkTheme.toolReadFile
        case "write_file", "write": return CyberpunkTheme.toolWriteFile
        case "edit_file", "edit": return CyberpunkTheme.toolEditFile
        case "search", "grep", "glob": return CyberpunkTheme.toolSearch
        case let name where name.starts(with: "mcp"): return CyberpunkTheme.toolMCP
        default: return CyberpunkTheme.accentMagenta
        }
    }

    private var toolIcon: String {
        switch toolName.lowercased() {
        case "bash": return "terminal"
        case "read_file", "read": return "doc.text"
        case "write_file", "write": return "doc.badge.plus"
        case "edit_file", "edit": return "pencil"
        case "search", "grep", "glob": return "magnifyingglass"
        case let name where name.starts(with: "mcp"): return "cpu"
        default: return "wrench"
        }
    }

    private var durationText: String {
        guard let ms = duration else { return "" }
        if ms < 1000 {
            return "\(ms)ms"
        } else {
            return String(format: "%.1fs", Double(ms) / 1000)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header row
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack(spacing: 12) {
                    // Tool icon
                    Image(systemName: toolIcon)
                        .font(.system(size: 16, weight: .medium))
                        .foregroundColor(toolColor)
                        .frame(width: 24)

                    // Tool name
                    Text(toolName)
                        .font(.system(.subheadline, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    Spacer()

                    // Duration
                    if !durationText.isEmpty {
                        Text(durationText)
                            .font(.caption.monospaced())
                            .foregroundColor(CyberpunkTheme.textMuted)
                    }

                    // Status indicator
                    StatusIndicator(status: status)

                    // Expand chevron
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
            }
            .buttonStyle(.plain)

            // Expanded content
            if isExpanded {
                VStack(alignment: .leading, spacing: 8) {
                    Divider()
                        .background(CyberpunkTheme.border)

                    // Input section
                    if let input = input {
                        VStack(alignment: .leading, spacing: 4) {
                            Label("Input", systemImage: "arrow.right.circle")
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.textSecondary)

                            Text(formatJSON(input))
                                .font(.system(.caption, design: .monospaced))
                                .foregroundColor(CyberpunkTheme.accentCyan)
                                .padding(8)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(CyberpunkTheme.bgPrimary)
                                .cornerRadius(4)
                        }
                    }

                    // Output section
                    if let output = output, !output.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            Label("Output", systemImage: "arrow.left.circle")
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.textSecondary)

                            Text(output)
                                .font(.system(.caption, design: .monospaced))
                                .foregroundColor(CyberpunkTheme.accentGreen)
                                .lineLimit(10)
                                .padding(8)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(CyberpunkTheme.bgPrimary)
                                .cornerRadius(4)
                        }
                    }
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 10)
            }
        }
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(
                    status == .running ? toolColor.opacity(0.5) : CyberpunkTheme.border,
                    lineWidth: 1
                )
        )
        .shadow(color: status == .running ? toolColor.opacity(0.2) : .clear, radius: 5)
    }

    private func formatJSON(_ dict: [String: Any]) -> String {
        if let data = try? JSONSerialization.data(withJSONObject: dict, options: .prettyPrinted),
           let string = String(data: data, encoding: .utf8) {
            return string
        }
        return dict.description
    }
}

// MARK: - Control Bar

/// Play/Pause/Stop controls for route execution
/// Matches ControlBar from ralph-mobile-prototype.jsx
struct ControlBar: View {
    enum RunState {
        case idle
        case running
        case paused
    }

    let state: RunState
    var onStart: () -> Void = {}
    var onPause: () -> Void = {}
    var onResume: () -> Void = {}
    var onStop: () -> Void = {}

    var body: some View {
        HStack(spacing: 16) {
            // Play/Pause button
            Button {
                switch state {
                case .idle:
                    onStart()
                case .running:
                    onPause()
                case .paused:
                    onResume()
                }
            } label: {
                Image(systemName: state == .running ? "pause.fill" : "play.fill")
                    .font(.system(size: 20, weight: .semibold))
                    .foregroundColor(CyberpunkTheme.bgPrimary)
                    .frame(width: 44, height: 44)
                    .background(
                        state == .running
                            ? CyberpunkTheme.accentYellow
                            : CyberpunkTheme.accentGreen
                    )
                    .clipShape(Circle())
                    .shadow(
                        color: (state == .running ? CyberpunkTheme.accentYellow : CyberpunkTheme.accentGreen).opacity(0.5),
                        radius: 8
                    )
            }
            .accessibilityIdentifier(
                state == .idle ? "dashboard-button-start" :
                state == .running ? "dashboard-button-pause" :
                "dashboard-button-resume"
            )

            // Stop button
            Button {
                onStop()
            } label: {
                Image(systemName: "stop.fill")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundColor(state == .idle ? CyberpunkTheme.textMuted : CyberpunkTheme.accentRed)
                    .frame(width: 36, height: 36)
                    .background(CyberpunkTheme.bgTertiary)
                    .clipShape(Circle())
                    .overlay(
                        Circle()
                            .stroke(
                                state == .idle ? CyberpunkTheme.border : CyberpunkTheme.accentRed.opacity(0.5),
                                lineWidth: 1
                            )
                    )
            }
            .disabled(state == .idle)
            .accessibilityIdentifier("dashboard-button-stop")
        }
    }
}

// MARK: - Backpressure Status View

/// Shows build/test/lint status checks
struct BackpressureStatusView: View {
    struct Check {
        let name: String
        let passed: Bool
    }

    let checks: [Check]

    var body: some View {
        HStack(spacing: 12) {
            ForEach(checks, id: \.name) { check in
                HStack(spacing: 4) {
                    Image(systemName: check.passed ? "checkmark.circle.fill" : "xmark.circle.fill")
                        .foregroundColor(check.passed ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentRed)
                        .font(.caption)

                    Text(check.name)
                        .font(.caption.monospaced())
                        .foregroundColor(CyberpunkTheme.textSecondary)
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(CyberpunkTheme.bgTertiary)
        .cornerRadius(8)
    }
}

// MARK: - User Steering FAB

/// Floating action button for user steering
/// Implements the FAB + in-event injection pattern from Q2 decision
struct UserSteeringFAB: View {
    @Binding var isPresented: Bool

    var body: some View {
        Button {
            isPresented = true
        } label: {
            Image(systemName: "message.badge.filled.fill")
                .font(.system(size: 22, weight: .semibold))
                .foregroundColor(CyberpunkTheme.bgPrimary)
                .frame(width: 56, height: 56)
                .background(CyberpunkTheme.accentPurple)
                .clipShape(Circle())
                .shadow(color: CyberpunkTheme.accentPurple.opacity(0.5), radius: 10)
        }
        .pulsingGlow(CyberpunkTheme.accentPurple)
    }
}

// MARK: - Preview

#Preview("Components") {
    ScrollView {
        VStack(spacing: 20) {
            HatStatusCard(
                hatName: "Builder",
                iteration: 3,
                triggerEvent: "plan.done",
                publishedEvents: ["build.done", "build.blocked"]
            )

            ToolCallCard(
                toolName: "bash",
                status: .running,
                input: ["command": "npm install"],
                output: nil,
                duration: nil,
                timestamp: Date()
            )

            ToolCallCard(
                toolName: "write_file",
                status: .completed,
                input: ["path": "src/index.ts"],
                output: "File written successfully",
                duration: 45,
                timestamp: Date()
            )

            ControlBar(state: .running)

            BackpressureStatusView(checks: [
                .init(name: "tests", passed: true),
                .init(name: "lint", passed: true),
                .init(name: "typecheck", passed: false)
            ])
        }
        .padding()
    }
    .background(CyberpunkTheme.bgPrimary)
}
