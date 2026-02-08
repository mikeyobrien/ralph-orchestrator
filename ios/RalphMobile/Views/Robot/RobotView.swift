import SwiftUI

/// View for the RObot human-in-the-loop interface.
/// Shows pending questions and allows sending guidance to running sessions.
struct RobotView: View {
    @StateObject private var viewModel = RobotViewModel()

    var body: some View {
        VStack(spacing: 0) {
            headerView
            contentView
            guidanceBar
        }
        .background(CyberpunkTheme.bgPrimary)
        .task {
            viewModel.startPolling()
        }
        .onDisappear {
            viewModel.stopPolling()
        }
    }

    // MARK: - Header

    private var headerView: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("ROBOT")
                    .font(.system(.headline, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentCyan)

                Text("Human-in-the-loop")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
            }

            Spacer()

            // Pending count badge
            if !viewModel.questions.isEmpty {
                Text("\(viewModel.questions.count)")
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(.white)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(CyberpunkTheme.accentRed)
                    .cornerRadius(12)
            }
        }
        .padding()
        .background(CyberpunkTheme.bgSecondary)
    }

    // MARK: - Content

    @ViewBuilder
    private var contentView: some View {
        if viewModel.isLoading && viewModel.questions.isEmpty {
            loadingView
        } else if let error = viewModel.error, viewModel.questions.isEmpty {
            errorView(error)
        } else if viewModel.questions.isEmpty {
            emptyView
        } else {
            questionsList
        }
    }

    private var questionsList: some View {
        ScrollView {
            LazyVStack(spacing: 8) {
                ForEach(viewModel.questions) { question in
                    QuestionCard(question: question) { response in
                        Task {
                            await viewModel.respondToQuestion(
                                questionId: question.id,
                                responseText: response
                            )
                        }
                    }
                    .accessibilityIdentifier("robot-question-\(question.id)")
                }
            }
            .padding()
        }
    }

    // MARK: - Guidance Bar

    private var guidanceBar: some View {
        VStack(spacing: 0) {
            Divider()
                .background(CyberpunkTheme.border)

            VStack(spacing: 8) {
                if !viewModel.availableSessions.isEmpty {
                    Picker("Session", selection: $viewModel.selectedSessionId) {
                        Text("Select session").tag(nil as String?)
                        ForEach(viewModel.availableSessions) { session in
                            Text(session.id.prefix(8) + (session.status == "running" ? " (running)" : ""))
                                .tag(session.id as String?)
                        }
                    }
                    .pickerStyle(.menu)
                    .tint(CyberpunkTheme.accentCyan)
                } else {
                    Text("No active sessions")
                        .font(.caption)
                        .foregroundColor(CyberpunkTheme.textMuted)
                }

                HStack(spacing: 8) {
                    TextField("Send guidance...", text: $viewModel.guidanceText)
                        .font(.system(.body, design: .monospaced))
                        .foregroundColor(CyberpunkTheme.textPrimary)
                        .padding(10)
                        .background(CyberpunkTheme.bgTertiary)
                        .cornerRadius(8)
                        .accessibilityIdentifier("robot-guidance-input")

                    Button {
                        Task { await viewModel.sendGuidance() }
                    } label: {
                        Image(systemName: "paperplane.fill")
                            .foregroundColor(
                                viewModel.guidanceText.isEmpty || viewModel.selectedSessionId == nil
                                    ? CyberpunkTheme.textMuted
                                    : CyberpunkTheme.accentCyan
                            )
                    }
                    .disabled(viewModel.guidanceText.isEmpty || viewModel.selectedSessionId == nil)
                    .accessibilityIdentifier("robot-guidance-send")
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            .background(CyberpunkTheme.bgSecondary)
        }
    }

    // MARK: - States

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .progressViewStyle(CircularProgressViewStyle(tint: CyberpunkTheme.accentCyan))
                .scaleEffect(1.5)
            Text("Checking for questions...")
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
            Button("Retry") {
                Task { await viewModel.fetchQuestions() }
            }
            .font(.system(.body, design: .monospaced).bold())
            .foregroundColor(CyberpunkTheme.accentCyan)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyView: some View {
        VStack(spacing: 16) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: 48))
                .foregroundColor(CyberpunkTheme.textMuted)
            Text("No pending questions")
                .font(.system(.headline, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textSecondary)
            Text("Questions from running agents appear here")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Question Card

private struct QuestionCard: View {
    let question: PendingQuestion
    let onRespond: (String) -> Void

    @State private var responseText: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            // Question header
            HStack {
                Image(systemName: "questionmark.circle.fill")
                    .foregroundColor(CyberpunkTheme.accentYellow)

                Text("Question")
                    .font(.system(.caption, design: .monospaced).bold())
                    .foregroundColor(CyberpunkTheme.accentYellow)

                Spacer()

                Text(question.sessionId)
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            // Question text
            Text(question.questionText)
                .font(.system(.body, design: .monospaced))
                .foregroundColor(CyberpunkTheme.textPrimary)

            // Response input
            HStack(spacing: 8) {
                TextField("Your response...", text: $responseText)
                    .font(.system(.caption, design: .monospaced))
                    .foregroundColor(CyberpunkTheme.textPrimary)
                    .padding(8)
                    .background(CyberpunkTheme.bgTertiary)
                    .cornerRadius(6)
                    .accessibilityIdentifier("robot-response-\(question.id)")

                Button {
                    onRespond(responseText)
                    responseText = ""
                } label: {
                    Text("Reply")
                        .font(.system(.caption, design: .monospaced).bold())
                        .foregroundColor(CyberpunkTheme.bgPrimary)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(
                            responseText.isEmpty
                                ? CyberpunkTheme.textMuted
                                : CyberpunkTheme.accentCyan
                        )
                        .cornerRadius(6)
                }
                .disabled(responseText.isEmpty)
                .accessibilityIdentifier("robot-reply-\(question.id)")
            }
        }
        .padding(12)
        .background(CyberpunkTheme.bgSecondary)
        .cornerRadius(8)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(CyberpunkTheme.accentYellow.opacity(0.3), lineWidth: 1)
        )
    }
}
