import SwiftUI

/// Displays real-time token usage and cost metrics with cyberpunk styling.
struct TokenMetricsView: View {
    let metrics: TokenMetrics

    // Cyberpunk theme colors
    private let backgroundColor = Color(red: 0.102, green: 0.102, blue: 0.180) // #1a1a2e
    private let accentColor = Color(red: 0.0, green: 0.831, blue: 1.0) // #00d4ff

    var body: some View {
        VStack(spacing: 12) {
            // Token counts row
            HStack(spacing: 16) {
                metricItem(label: "Input", value: formatNumber(metrics.inputTokens))
                    .accessibilityIdentifier("token-metrics-input")
                metricItem(label: "Output", value: formatNumber(metrics.outputTokens))
                    .accessibilityIdentifier("token-metrics-output")
                metricItem(label: "Total", value: formatNumber(metrics.totalTokens), isHighlighted: true)
                    .accessibilityIdentifier("token-metrics-total")
            }

            // Cost and duration row (when available)
            if metrics.estimatedCost > 0 || metrics.durationMs != nil {
                Divider()
                    .background(accentColor.opacity(0.3))

                HStack(spacing: 16) {
                    if metrics.estimatedCost > 0 {
                        metricItem(label: "Cost", value: formatCost(metrics.estimatedCost))
                            .accessibilityIdentifier("token-metrics-cost")
                    } else {
                        metricItem(label: "Cost", value: "--")
                            .accessibilityIdentifier("token-metrics-cost")
                    }

                    if let duration = metrics.durationMs {
                        metricItem(label: "Duration", value: formatDuration(duration))
                            .accessibilityIdentifier("token-metrics-duration")
                    } else {
                        metricItem(label: "Duration", value: "--")
                            .accessibilityIdentifier("token-metrics-duration")
                    }
                }
            }
        }
        .padding(16)
        .background(backgroundColor)
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(accentColor.opacity(0.3), lineWidth: 1)
        )
        .accessibilityIdentifier("token-metrics-view")
    }

    /// Individual metric item with label and value.
    private func metricItem(label: String, value: String, isHighlighted: Bool = false) -> some View {
        VStack(spacing: 4) {
            Text(label)
                .font(.caption)
                .foregroundColor(accentColor.opacity(0.8))
                .textCase(.uppercase)

            Text(value)
                .font(.system(.title3, design: .monospaced))
                .fontWeight(isHighlighted ? .bold : .medium)
                .foregroundColor(isHighlighted ? accentColor : .white)
        }
        .frame(maxWidth: .infinity)
    }

    /// Format number with thousands separator.
    private func formatNumber(_ value: Int) -> String {
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        formatter.groupingSeparator = ","
        return formatter.string(from: NSNumber(value: value)) ?? "\(value)"
    }

    /// Format cost as USD currency.
    private func formatCost(_ value: Double) -> String {
        let formatter = NumberFormatter()
        formatter.numberStyle = .currency
        formatter.currencyCode = "USD"
        formatter.maximumFractionDigits = value < 0.01 ? 4 : 2
        return formatter.string(from: NSNumber(value: value)) ?? "$\(value)"
    }

    /// Format duration in human-readable form.
    private func formatDuration(_ ms: Int) -> String {
        let totalSeconds = ms / 1000
        let minutes = totalSeconds / 60
        let seconds = totalSeconds % 60

        if minutes > 0 {
            return "\(minutes)m \(seconds)s"
        } else {
            return "\(seconds)s"
        }
    }
}

#Preview("With All Metrics") {
    TokenMetricsView(metrics: TokenMetrics(
        inputTokens: 15234,
        outputTokens: 7891,
        estimatedCost: 0.0523,
        durationMs: 125000
    ))
    .padding()
    .background(Color.black)
}

#Preview("Tokens Only") {
    TokenMetricsView(metrics: TokenMetrics(
        inputTokens: 1500,
        outputTokens: 750,
        estimatedCost: 0.0,
        durationMs: nil
    ))
    .padding()
    .background(Color.black)
}

#Preview("Empty State") {
    TokenMetricsView(metrics: TokenMetrics())
    .padding()
    .background(Color.black)
}
