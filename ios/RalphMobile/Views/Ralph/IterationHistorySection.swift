import SwiftUI

/// Displays iteration history for a Ralph session as a collapsible section.
/// Shows iteration number, hat emoji, duration, and relative time.
struct IterationHistorySection: View {
    let iterations: [IterationItem]
    let isLoading: Bool
    @Binding var isExpanded: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Collapsible header
            Button {
                withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack {
                    Image(systemName: "clock.arrow.circlepath")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.accentPurple)

                    Text("ITERATION HISTORY")
                        .font(.system(.caption, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textSecondary)

                    Text("(\(iterations.count))")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)

                    Spacer()

                    Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
                .padding(12)
                .background(CyberpunkTheme.bgSecondary)
                .cornerRadius(8)
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("iteration-history-toggle")

            if isExpanded {
                if isLoading {
                    HStack {
                        Spacer()
                        ProgressView()
                            .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                        Spacer()
                    }
                    .padding()
                } else if iterations.isEmpty {
                    Text("No iterations yet")
                        .font(.system(.caption, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .frame(maxWidth: .infinity)
                        .padding()
                } else {
                    LazyVStack(spacing: 4) {
                        ForEach(iterations.prefix(200)) { iteration in
                            IterationRowView(iteration: iteration)
                                .accessibilityIdentifier("iteration-item-\(iteration.number)")
                        }
                    }
                    .padding(.top, 4)
                }
            }
        }
    }
}

// MARK: - Iteration Row

private struct IterationRowView: View {
    let iteration: IterationItem

    var body: some View {
        HStack(spacing: 10) {
            // Iteration number
            Text("#\(iteration.number)")
                .font(.system(.caption, design: .monospaced).bold())
                .foregroundColor(CyberpunkTheme.accentCyan)
                .frame(width: 36, alignment: .leading)

            // Hat name
            Text(iteration.hat ?? "—")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textPrimary)

            Spacer()

            // Duration
            if let secs = iteration.durationSecs {
                Text("\(secs)s")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            // Status indicator
            Circle()
                .fill(statusColor)
                .frame(width: 6, height: 6)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(4)
    }

    private var statusColor: Color {
        // Derive status from available fields: if durationSecs exists, it completed
        if iteration.durationSecs != nil {
            return CyberpunkTheme.accentGreen
        } else {
            return CyberpunkTheme.accentCyan // still running
        }
    }
}
