# Rough Idea

Ensure tasks and hats primitives are exposed such that one could build a Kanban-style UX to track workstreams (loops).

The goal is to surface enough structured data from Ralph's task, hat, and loop systems so that a frontend (web dashboard or external tool) can render a Kanban board where:
- Columns represent stages/states (e.g., task statuses, active hats, loop phases)
- Cards represent individual tasks or loop iterations
- Users can visualize and track multiple concurrent workstreams (parallel loops)
