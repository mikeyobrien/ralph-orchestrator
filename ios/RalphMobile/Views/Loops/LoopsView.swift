import SwiftUI

/// View for managing parallel orchestration loops and the merge queue.
struct LoopsView: View {
    @StateObject private var viewModel = LoopsViewModel()
    @State private var showSpawnSheet = false

    var body: some View {
        VStack(spacing: 0) {
            headerView
            contentView
        }
        .background(CyberpunkTheme.bgPrimary)
        .task {
            await viewModel.fetchLoops()
            await viewModel.fetchMergeQueue()
        }
        .alert("Result", isPresented: .constant(viewModel.operationResult != nil)) {
            Button("OK") { viewModel.operationResult = nil }
        } message: {
            if let result = viewModel.operationResult {
                Text(result)
            }
        }
        .sheet(isPresented: $showSpawnSheet) {
            SpawnLoopSheet { prompt, configPath in
                Task {
                    await viewModel.spawnLoop(prompt: prompt, configPath: configPath)
                }
            }
        }
    }

    // MARK: - Header

    private var headerView: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("LOOPS")
                    .font(.system(.headline, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)

                Text("\(viewModel.loops.count) active")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            Spacer()

            Button {
                showSpawnSheet = true
            } label: {
                Image(systemName: "plus")
                    .foregroundColor(CyberpunkTheme.accentCyan)
            }

            Button {
                Task { await viewModel.fetchLoops() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.body)
                    .foregroundColor(CyberpunkTheme.textSecondary)
            }
        }
        .padding()
        .background(CyberpunkTheme.bgSecondary)
    }

    // MARK: - Content

    @ViewBuilder
    private var contentView: some View {
        if viewModel.isLoading {
            loadingView
        } else if let error = viewModel.error {
            errorView(error)
        } else if viewModel.loops.isEmpty {
            emptyView
        } else {
            loopsList
        }
    }

    private var loopsList: some View {
        ScrollView {
            LazyVStack(spacing: 8) {
                // Primary loop
                if let primary = viewModel.primaryLoop {
                    loopCard(loop: primary, isPrimary: true)
                }

                // Worktree loops
                if !viewModel.worktreeLoops.isEmpty {
                    Text("WORKTREE LOOPS")
                        .font(.system(.caption2, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, 8)

                    ForEach(viewModel.worktreeLoops) { loop in
                        loopCard(loop: loop, isPrimary: false)
                    }
                }

                // Merge queue
                if let mq = viewModel.mergeQueue, !mq.pending.isEmpty {
                    Text("MERGE QUEUE")
                        .font(.system(.caption2, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.textMuted)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, 8)

                    ForEach(mq.pending) { item in
                        mergeQueueCard(item: item)
                    }
                }
            }
            .padding()
        }
        .refreshable {
            await viewModel.fetchLoops()
            await viewModel.fetchMergeQueue()
        }
    }

    private func loopCard(loop: LoopInfo, isPrimary: Bool) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                // Status badge
                Text(isPrimary ? "PRIMARY" : "WORKTREE")
                    .font(.system(.caption2, design: .monospaced).bold())
                    .foregroundColor(isPrimary ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentPurple)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background((isPrimary ? CyberpunkTheme.accentGreen : CyberpunkTheme.accentPurple).opacity(0.2))
                    .cornerRadius(4)

                Spacer()

                if !isPrimary {
                    // Merge/Discard buttons for worktree loops
                    HStack(spacing: 8) {
                        Button {
                            Task { await viewModel.mergeLoop(id: loop.id) }
                        } label: {
                            Image(systemName: "arrow.triangle.merge")
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.accentGreen)
                        }
                        .accessibilityIdentifier("loop-merge-\(loop.id)")

                        Button {
                            Task { await viewModel.discardLoop(id: loop.id) }
                        } label: {
                            Image(systemName: "trash")
                                .font(.caption)
                                .foregroundColor(CyberpunkTheme.accentRed)
                        }
                        .accessibilityIdentifier("loop-discard-\(loop.id)")
                    }
                }
            }

            Text(loop.id)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textPrimary)
                .lineLimit(1)
                .truncationMode(.middle)

            Text(loop.prompt)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
                .lineLimit(2)

            HStack(spacing: 16) {
                Label("PID \(loop.pid)", systemImage: "gearshape")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)

                Label(loop.workspace, systemImage: "folder")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .lineLimit(1)
            }
        }
        .padding(12)
        .background(CyberpunkTheme.bgSecondary)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
        .accessibilityIdentifier("loops-item-\(loop.id)")
    }

    private func mergeQueueCard(item: MergeQueueItem) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text(item.id)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textPrimary)
                    .lineLimit(1)
                    .truncationMode(.middle)

                Text(item.status)
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            Spacer()

            Circle()
                .fill(item.status == "pending" ? CyberpunkTheme.accentYellow : CyberpunkTheme.accentGreen)
                .frame(width: 8, height: 8)
        }
        .padding(12)
        .background(CyberpunkTheme.bgCard)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.border, lineWidth: 1)
        )
        .accessibilityIdentifier("merge-queue-item-\(item.id)")
    }

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)
            Text("Loading loops...")
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func errorView(_ message: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.accentRed)
            Text(message)
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
                .multilineTextAlignment(.center)
            Button("Retry") {
                Task { await viewModel.fetchLoops() }
            }
            .font(.system(.body, design: .monospaced).bold())
            .foregroundColor(CyberpunkTheme.accentCyan)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyView: some View {
        VStack(spacing: 16) {
            Image(systemName: "arrow.triangle.branch")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.textMuted)
            Text("No active loops")
                .font(.system(.headline, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
            Text("Start a Ralph run to see loops here")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
