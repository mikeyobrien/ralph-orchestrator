# Broken Windows — Ralph Memories Implementation

## Summary

After reviewing the files that will be modified (`main.rs`, `lib.rs`, `Cargo.toml`), **no broken windows were identified** that meet the criteria for low-risk opportunistic fixes.

## Files Reviewed

### crates/ralph-cli/src/main.rs

- **Code Quality**: Well-organized with clear separation between command definitions and handlers
- **Naming**: Consistent use of `_command` suffix for handler functions
- **Comments**: Adequate doc comments on public items
- **No Issues Found**: The file follows established patterns consistently

### crates/ralph-core/src/lib.rs

- **Code Quality**: Clean module organization with explicit re-exports
- **No Issues Found**: Standard module structure, no dead code or inconsistencies

### crates/ralph-core/Cargo.toml

- **Dependencies**: All necessary dependencies present (except `rand` which will be added)
- **No Issues Found**: Standard Cargo.toml structure

### crates/ralph-cli/Cargo.toml

- **Dependencies**: All necessary dependencies present
- **No Issues Found**: Standard Cargo.toml structure

## Notes for Implementation

### Design Document Gaps (from Design Critic notes)

The following items were flagged as "minor notes" during design review and should be addressed during implementation:

1. **`rand` crate dependency** — Must be added to workspace `Cargo.toml` and `ralph-core/Cargo.toml`

2. **`MemoryType::as_str()` method** — The design shows `memory.memory_type.as_str()` in the table formatter but doesn't define the method. Implement as:
   ```rust
   impl MemoryType {
       pub fn as_str(&self) -> &'static str {
           match self {
               MemoryType::Pattern => "pattern",
               MemoryType::Decision => "decision",
               MemoryType::Fix => "fix",
               MemoryType::Context => "context",
           }
       }
   }
   ```

3. **`OutputFormat` enum** — Already exists in `main.rs:161-168`, can be reused for memories commands

These are not broken windows but design details to complete during implementation.
