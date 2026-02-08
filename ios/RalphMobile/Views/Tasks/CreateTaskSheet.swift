import SwiftUI

/// Sheet for creating a new orchestration task.
struct CreateTaskSheet: View {
    let onCreate: (String, String?, UInt8) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var title: String = ""
    @State private var description: String = ""
    @State private var priority: Int = 3

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Task title", text: $title)
                        .font(.system(.body, design: .monospaced))
                        .accessibilityIdentifier("create-task-title")
                } header: {
                    Text("Title")
                        .foregroundColor(CyberpunkTheme.textMuted)
                }

                Section {
                    TextEditor(text: $description)
                        .font(.system(.body, design: .monospaced))
                        .frame(minHeight: 80)
                        .accessibilityIdentifier("create-task-description")
                } header: {
                    Text("Description (optional)")
                        .foregroundColor(CyberpunkTheme.textMuted)
                }

                Section {
                    Stepper("Priority: P\(priority)", value: $priority, in: 1...5)
                        .font(.system(.body, design: .monospaced))
                        .accessibilityIdentifier("create-task-priority")
                } header: {
                    Text("Priority")
                        .foregroundColor(CyberpunkTheme.textMuted)
                } footer: {
                    Text("P1 = Critical, P5 = Low")
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
            }
            .scrollContentBackground(.hidden)
            .background(CyberpunkTheme.bgPrimary)
            .navigationTitle("New Task")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .foregroundColor(CyberpunkTheme.textMuted)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") {
                        let desc = description.isEmpty ? nil : description
                        onCreate(title, desc, UInt8(priority))
                    }
                    .disabled(title.isEmpty)
                    .foregroundColor(title.isEmpty ? CyberpunkTheme.textMuted : CyberpunkTheme.accentCyan)
                    .accessibilityIdentifier("create-task-submit")
                }
            }
        }
    }
}
