import SwiftUI

/// Hat Flow visualization showing the workflow pipeline
/// Each hat is tappable to show detailed information
struct HatFlowView: View {
    let currentHat: String
    let iteration: Int
    let triggerEvent: String?
    let publishedEvents: [String]

    @State private var selectedHat: HatInfo? = nil

    // Standard hat flow from V3 architecture
    private let hatFlow: [HatInfo] = [
        HatInfo(
            name: "planner",
            emoji: "📋",
            color: Color(hex: "#3b82f6"),
            triggers: ["design.approved", "context.ready"],
            publishes: ["plan.ready"]
        ),
        HatInfo(
            name: "builder",
            emoji: "🔨",
            color: Color(hex: "#00fff2"),
            triggers: ["tasks.ready", "validation.failed", "task.complete"],
            publishes: ["implementation.ready", "build.blocked", "task.complete"]
        ),
        HatInfo(
            name: "reviewer",
            emoji: "👁️",
            color: Color(hex: "#a855f7"),
            triggers: ["implementation.ready"],
            publishes: ["review.passed", "review.failed"]
        ),
        HatInfo(
            name: "tester",
            emoji: "🧪",
            color: Color(hex: "#00ff88"),
            triggers: ["review.passed"],
            publishes: ["validation.passed", "validation.failed"]
        )
    ]

    var body: some View {
        VStack(spacing: 12) {
            // Hat pipeline
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(hatFlow, id: \.name) { hat in
                        HatCard(
                            hat: hat,
                            isActive: isActive(hat),
                            activations: activationsFor(hat),
                            maxActivations: maxActivationsFor(hat)
                        )
                        .accessibilityIdentifier("hatflow-card-\(hat.name)")
                        .onTapGesture {
                            withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                                if selectedHat?.name == hat.name {
                                    selectedHat = nil
                                } else {
                                    selectedHat = hat
                                }
                            }
                        }

                        if hat.name != hatFlow.last?.name {
                            Image(systemName: "arrow.right")
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.textMuted)
                        }
                    }
                }
            }

            // Expanded hat detail
            if let selected = selectedHat {
                HatDetailCard(hat: selected, isActive: isActive(selected))
                    .accessibilityIdentifier("hatflow-detail-\(selected.name)")
                    .transition(.opacity.combined(with: .scale(scale: 0.95)))
            }
        }
    }

    private func isActive(_ hat: HatInfo) -> Bool {
        currentHat.lowercased().contains(hat.name.lowercased())
    }

    private func activationsFor(_ hat: HatInfo) -> Int {
        // Simulate activations based on iteration
        if isActive(hat) { return min(iteration, 10) }
        return max(0, iteration - 1)
    }

    private func maxActivationsFor(_ hat: HatInfo) -> Int {
        switch hat.name {
        case "planner": return 5
        case "builder": return 10
        case "reviewer": return 5
        case "tester": return 5
        default: return 5
        }
    }
}

/// Individual hat in the flow
struct HatInfo: Identifiable {
    var id: String { name }
    let name: String
    let emoji: String
    let color: Color
    let triggers: [String]
    let publishes: [String]
}

/// Small card showing hat in the pipeline
private struct HatCard: View {
    let hat: HatInfo
    let isActive: Bool
    let activations: Int
    let maxActivations: Int

    var body: some View {
        VStack(spacing: 4) {
            Text(hat.emoji)
                .font(.title2)

            Text("\(activations)/\(maxActivations)")
                .font(.system(.caption2, design: .monospaced))
                .foregroundColor(isActive ? CyberpunkTheme.textPrimary : CyberpunkTheme.textMuted)

            if isActive {
                Text("ACTV")
                    .font(.system(.caption2, design: .monospaced).bold())
                    .foregroundColor(hat.color)
            }
        }
        .frame(width: 60, height: 70)
        .background(CyberpunkTheme.bgTertiary)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isActive ? hat.color : CyberpunkTheme.border, lineWidth: isActive ? 2 : 1)
        )
        .shadow(color: isActive ? hat.color.opacity(0.4) : .clear, radius: 8)
    }
}

/// Expanded detail card for selected hat
struct HatDetailCard: View {
    let hat: HatInfo
    let isActive: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header
            HStack(spacing: 12) {
                Text(hat.emoji)
                    .font(.title)

                VStack(alignment: .leading, spacing: 2) {
                    Text(hat.name.capitalized)
                        .font(.headline)
                        .foregroundColor(CyberpunkTheme.textPrimary)

                    HStack(spacing: 4) {
                        Circle()
                            .fill(isActive ? hat.color : CyberpunkTheme.textMuted)
                            .frame(width: 6, height: 6)

                        Text(isActive ? "Active" : "Idle")
                            .font(.caption)
                            .foregroundColor(CyberpunkTheme.textSecondary)
                    }
                }

                Spacer()
            }

            Divider()
                .background(CyberpunkTheme.border)

            // Triggers
            VStack(alignment: .leading, spacing: 4) {
                Label("Triggers", systemImage: "bolt.fill")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.accentYellow)

                FlowLayout(spacing: 4) {
                    ForEach(hat.triggers, id: \.self) { trigger in
                        Text(trigger)
                            .font(.caption.monospaced())
                            .foregroundColor(CyberpunkTheme.accentYellow)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(CyberpunkTheme.accentYellow.opacity(0.15))
                            .cornerRadius(4)
                    }
                }
            }

            // Publishes
            VStack(alignment: .leading, spacing: 4) {
                Label("Publishes", systemImage: "arrow.up.circle.fill")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.accentGreen)

                FlowLayout(spacing: 4) {
                    ForEach(hat.publishes, id: \.self) { event in
                        Text(event)
                            .font(.caption.monospaced())
                            .foregroundColor(CyberpunkTheme.accentGreen)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(CyberpunkTheme.accentGreen.opacity(0.15))
                            .cornerRadius(4)
                    }
                }
            }
        }
        .padding()
        .background(CyberpunkTheme.bgTertiary)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(hat.color.opacity(0.5), lineWidth: 1)
        )
    }
}

/// Simple flow layout for tags
struct FlowLayout: Layout {
    var spacing: CGFloat = 4

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = arrangeSubviews(proposal: proposal, subviews: subviews)
        return result.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = arrangeSubviews(proposal: proposal, subviews: subviews)

        for (index, subview) in subviews.enumerated() {
            if index < result.positions.count {
                subview.place(at: CGPoint(
                    x: bounds.minX + result.positions[index].x,
                    y: bounds.minY + result.positions[index].y
                ), proposal: .unspecified)
            }
        }
    }

    private func arrangeSubviews(proposal: ProposedViewSize, subviews: Subviews) -> (size: CGSize, positions: [CGPoint]) {
        let maxWidth = proposal.width ?? .infinity
        var positions: [CGPoint] = []
        var currentX: CGFloat = 0
        var currentY: CGFloat = 0
        var lineHeight: CGFloat = 0
        var maxX: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)

            if currentX + size.width > maxWidth && currentX > 0 {
                currentX = 0
                currentY += lineHeight + spacing
                lineHeight = 0
            }

            positions.append(CGPoint(x: currentX, y: currentY))
            lineHeight = max(lineHeight, size.height)
            currentX += size.width + spacing
            maxX = max(maxX, currentX)
        }

        return (CGSize(width: maxX, height: currentY + lineHeight), positions)
    }
}

#Preview {
    HatFlowView(
        currentHat: "builder",
        iteration: 5,
        triggerEvent: "plan.ready",
        publishedEvents: ["build.done"]
    )
    .padding()
    .background(CyberpunkTheme.bgPrimary)
}
