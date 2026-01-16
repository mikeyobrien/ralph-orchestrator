# Rough Idea: SOP Injection Subcommands

Add two new subcommands to ralph-orchestrator:

1. **`ralph plan`** - Injects the PDD (Prompt-Driven Development) agent SOP into the user's default backend
2. **`ralph task`** - Injects the code-task-generator agent SOP into the user's default backend

These commands allow users to quickly start structured workflows (planning or task generation) using their configured default backend without needing to manually set up the SOP context.
