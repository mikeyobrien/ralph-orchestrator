import SwiftUI

/// View model for host metrics with auto-refresh.
@MainActor
class HostMetricsViewModel: ObservableObject {
    @Published var metrics: HostMetricsResponse?
    @Published var processes: [ProcessInfo] = []
    @Published var isLoading = false
    @Published var error: String?
    @Published var lastUpdated: Date?

    private var refreshTask: Task<Void, Never>?

    func startAutoRefresh() {
        refreshTask?.cancel()
        refreshTask = Task {
            while !Task.isCancelled {
                await fetchMetrics()
                try? await Task.sleep(nanoseconds: 3_000_000_000) // 3 seconds
            }
        }
    }

    func stopAutoRefresh() {
        refreshTask?.cancel()
        refreshTask = nil
    }

    func fetchMetrics() async {
        guard RalphAPIClient.isConfigured else {
            error = "Server not configured"
            return
        }

        isLoading = true
        error = nil

        do {
            async let metricsTask = RalphAPIClient.shared.getHostMetrics()
            async let processesTask = RalphAPIClient.shared.getHostProcesses()

            let (fetchedMetrics, fetchedProcesses) = try await (metricsTask, processesTask)

            metrics = fetchedMetrics
            processes = fetchedProcesses
            lastUpdated = Date()
            error = nil
        } catch {
            self.error = error.localizedDescription
        }

        isLoading = false
    }
}

/// Host metrics view - shows real-time system metrics from the backend.
struct HostMetricsView: View {
    @StateObject private var viewModel = HostMetricsViewModel()

    var body: some View {
        ScrollView {
            VStack(spacing: 20) {
                if let error = viewModel.error {
                    errorView(error)
                } else if let metrics = viewModel.metrics {
                    metricsContent(metrics)
                } else if viewModel.isLoading {
                    loadingView
                } else {
                    notConfiguredView
                }
            }
            .padding()
        }
        .background(CyberpunkTheme.bgPrimary)
        .navigationTitle("Host Metrics")
        .onAppear {
            viewModel.startAutoRefresh()
        }
        .onDisappear {
            viewModel.stopAutoRefresh()
        }
    }

    @ViewBuilder
    private func metricsContent(_ metrics: HostMetricsResponse) -> some View {
        // Last updated header
        if let lastUpdated = viewModel.lastUpdated {
            HStack {
                Image(systemName: "clock")
                    .foregroundColor(CyberpunkTheme.accentCyan)
                Text("Updated: \(lastUpdated.formatted(date: .omitted, time: .standard))")
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textSecondary)
                Spacer()
                if viewModel.isLoading {
                    ProgressView()
                        .scaleEffect(0.7)
                }
            }
            .padding(.bottom, 8)
        }

        // Metrics Grid
        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 16) {
            HostMetricCard(
                title: "CPU",
                value: String(format: "%.1f%%", metrics.cpu.usagePercent),
                subtitle: "\(metrics.cpu.cores) cores",
                icon: "cpu",
                color: colorForPercentage(metrics.cpu.usagePercent)
            )

            HostMetricCard(
                title: "Memory",
                value: String(format: "%.1f%%", metrics.memory.usagePercent),
                subtitle: String(format: "%.1f / %.1f GB", metrics.memory.usedGb, metrics.memory.totalGb),
                icon: "memorychip",
                color: colorForPercentage(metrics.memory.usagePercent)
            )

            HostMetricCard(
                title: "Disk",
                value: String(format: "%.1f%%", metrics.disk.usagePercent),
                subtitle: String(format: "%.0f / %.0f GB", metrics.disk.usedGb, metrics.disk.totalGb),
                icon: "internaldrive",
                color: colorForPercentage(metrics.disk.usagePercent)
            )

            HostMetricCard(
                title: "Network",
                value: String(format: "%.1f MB/s", metrics.network.downloadMbps),
                subtitle: String(format: "↑ %.1f MB/s", metrics.network.uploadMbps),
                icon: "network",
                color: CyberpunkTheme.accentCyan
            )
        }

        // Top Processes
        if !viewModel.processes.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                Text("Top Processes")
                    .font(.headline)
                    .foregroundColor(CyberpunkTheme.textPrimary)

                ForEach(viewModel.processes) { process in
                    ProcessRow(process: process)
                }
            }
            .padding()
            .background(CyberpunkTheme.bgSecondary)
            .cornerRadius(12)
        }
    }

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.5)
            Text("Loading metrics...")
                .foregroundColor(CyberpunkTheme.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.top, 100)
    }

    private var notConfiguredView: some View {
        VStack(spacing: 24) {
            Image(systemName: "server.rack")
                .font(.system(size: 64))
                .foregroundColor(CyberpunkTheme.textMuted)

            Text("Server Not Configured")
                .font(.title2.bold())
                .foregroundColor(CyberpunkTheme.textPrimary)

            Text("Configure the server URL in Settings to view host metrics.")
                .font(.body)
                .foregroundColor(CyberpunkTheme.textSecondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.top, 100)
    }

    private func errorView(_ error: String) -> some View {
        VStack(spacing: 24) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 64))
                .foregroundColor(CyberpunkTheme.accentRed)

            Text("Connection Error")
                .font(.title2.bold())
                .foregroundColor(CyberpunkTheme.textPrimary)

            Text(error)
                .font(.body)
                .foregroundColor(CyberpunkTheme.textSecondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            Button {
                Task { await viewModel.fetchMetrics() }
            } label: {
                Label("Retry", systemImage: "arrow.clockwise")
                    .padding(.horizontal, 24)
                    .padding(.vertical, 12)
                    .background(CyberpunkTheme.accentCyan)
                    .foregroundColor(.black)
                    .cornerRadius(8)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(.top, 100)
    }

    private func colorForPercentage(_ percent: Double) -> Color {
        if percent < 60 {
            return CyberpunkTheme.accentGreen
        } else if percent < 80 {
            return CyberpunkTheme.accentYellow
        } else {
            return CyberpunkTheme.accentRed
        }
    }
}

/// Card for displaying a single metric.
struct HostMetricCard: View {
    let title: String
    let value: String
    let subtitle: String
    let icon: String
    let color: Color

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: icon)
                    .foregroundColor(color)
                Text(title)
                    .font(.caption)
                    .foregroundColor(CyberpunkTheme.textSecondary)
            }

            Text(value)
                .font(.title2.bold())
                .foregroundColor(color)

            Text(subtitle)
                .font(.caption)
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding()
        .background(CyberpunkTheme.bgSecondary)
        .cornerRadius(12)
    }
}

/// Row for displaying process info.
struct ProcessRow: View {
    let process: ProcessInfo

    var body: some View {
        HStack {
            Text(process.name)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textPrimary)
                .lineLimit(1)

            Spacer()

            Text(String(format: "%.1f%%", process.cpuPercent))
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(colorForCpu(process.cpuPercent))
                .frame(width: 50, alignment: .trailing)

            Text(String(format: "%.0f MB", process.memoryMb))
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
                .frame(width: 70, alignment: .trailing)
        }
        .padding(.vertical, 4)
    }

    private func colorForCpu(_ percent: Double) -> Color {
        if percent < 10 {
            return CyberpunkTheme.textSecondary
        } else if percent < 50 {
            return CyberpunkTheme.accentYellow
        } else {
            return CyberpunkTheme.accentRed
        }
    }
}

#Preview {
    HostMetricsView()
}
