import SwiftUI

/// Common event topics for quick selection.
enum CommonEventTopic: String, CaseIterable, Identifiable {
    case buildDone = "build.done"
    case reviewComplete = "review.complete"
    case testsPass = "tests.pass"
    case testsFail = "tests.fail"
    case analysisDone = "analysis.done"
    case deployDone = "deploy.done"
    case custom = "custom"

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .buildDone: return "Build Done"
        case .reviewComplete: return "Review Complete"
        case .testsPass: return "Tests Pass"
        case .testsFail: return "Tests Fail"
        case .analysisDone: return "Analysis Done"
        case .deployDone: return "Deploy Done"
        case .custom: return "Custom..."
        }
    }
}

/// Sheet for emitting events to a running Ralph session.
///
/// Provides topic picker with common event types, optional JSON payload editor,
/// validation feedback, and submission with success/error states.
struct EventEmitSheet: View {
    let sessionId: String
    let apiClient: RalphAPIClient
    @Binding var isPresented: Bool

    @State private var selectedTopic: CommonEventTopic = .buildDone
    @State private var customTopic: String = ""
    @State private var payload: String = ""
    @State private var isSubmitting = false
    @State private var error: Error?
    @State private var lastEmittedTimestamp: String?
    @State private var emitHistory: [(topic: String, timestamp: String)] = []

    private var effectiveTopic: String {
        selectedTopic == .custom ? customTopic : selectedTopic.rawValue
    }

    private var isPayloadValid: Bool {
        if payload.isEmpty { return true }
        return (try? JSONSerialization.jsonObject(with: Data(payload.utf8))) != nil
    }

    private var canSubmit: Bool {
        !effectiveTopic.isEmpty && isPayloadValid && !isSubmitting
    }

    var body: some View {
        NavigationStack {
            Form {
                // Topic Selection
                Section("Event Topic") {
                    Picker("Topic", selection: $selectedTopic) {
                        ForEach(CommonEventTopic.allCases) { topic in
                            Text(topic.displayName).tag(topic)
                        }
                    }
                    .pickerStyle(.menu)

                    if selectedTopic == .custom {
                        TextField("Custom topic (e.g., my.event)", text: $customTopic)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                    }
                }

                // Payload Editor
                Section("Payload (Optional JSON)") {
                    TextEditor(text: $payload)
                        .font(.system(.body, design: .monospaced))
                        .frame(minHeight: 100)

                    if !payload.isEmpty && !isPayloadValid {
                        Label("Invalid JSON syntax", systemImage: "exclamationmark.triangle")
                            .foregroundColor(.red)
                    }
                }

                // Submit Button
                Section {
                    Button {
                        Task { await emitEvent() }
                    } label: {
                        HStack {
                            if isSubmitting {
                                ProgressView()
                                    .padding(.trailing, 8)
                            }
                            Text(isSubmitting ? "Emitting..." : "Emit Event")
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .disabled(!canSubmit)
                }

                // Success Feedback
                if let timestamp = lastEmittedTimestamp {
                    Section {
                        Label("Event emitted at \(formatTimestamp(timestamp))", systemImage: "checkmark.circle")
                            .foregroundColor(.green)
                    }
                }

                // Error Feedback
                if let error = error {
                    Section {
                        Label(error.localizedDescription, systemImage: "xmark.circle")
                            .foregroundColor(.red)
                    }
                }

                // Emit History
                if !emitHistory.isEmpty {
                    Section("Recent Emissions") {
                        ForEach(emitHistory, id: \.timestamp) { item in
                            Button {
                                // Quick re-emit - set topic
                                if let topic = CommonEventTopic(rawValue: item.topic) {
                                    selectedTopic = topic
                                } else {
                                    selectedTopic = .custom
                                    customTopic = item.topic
                                }
                            } label: {
                                VStack(alignment: .leading) {
                                    Text(item.topic)
                                        .font(.headline)
                                        .foregroundColor(.primary)
                                    Text(formatTimestamp(item.timestamp))
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                }
                            }
                        }
                    }
                }
            }
            .navigationTitle("Emit Event")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        isPresented = false
                    }
                }
            }
        }
    }

    private func emitEvent() async {
        isSubmitting = true
        error = nil
        lastEmittedTimestamp = nil

        do {
            let payloadString = payload.isEmpty ? nil : payload
            let response = try await apiClient.emitEvent(
                sessionId: sessionId,
                topic: effectiveTopic,
                payload: payloadString
            )
            lastEmittedTimestamp = response.timestamp
            emitHistory.insert((topic: effectiveTopic, timestamp: response.timestamp), at: 0)
            if emitHistory.count > 5 {
                emitHistory.removeLast()
            }
        } catch {
            self.error = error
        }

        isSubmitting = false
    }

    private func formatTimestamp(_ iso: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: iso) else {
            // Try without fractional seconds
            formatter.formatOptions = [.withInternetDateTime]
            guard let date = formatter.date(from: iso) else {
                return iso
            }
            return formatDate(date)
        }
        return formatDate(date)
    }

    private func formatDate(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .medium
        return formatter.string(from: date)
    }
}
