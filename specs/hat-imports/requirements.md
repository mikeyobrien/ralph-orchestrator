# Requirements — Hat Imports

## Questions & Answers

### Q1: Should Phase 1 be limited to local file imports only (no URL imports)?

Research found that URL imports introduce significant security complexity (lockfiles, caching, domain allowlists, content validation) because imported hats contain AI agent instructions with tool access. Local file imports alone deliver the core value — cross-preset hat reuse — with dramatically simpler implementation. URL imports could follow in a separate Phase 2.

**Answer: (a) Phase 1 = local file imports only. URL imports deferred to Phase 2.**

---

### Q2: Should imported hat files be allowed to contain their own `import:` directives (transitive imports)?

Allowing transitive imports enables deeper composition (e.g., a shared builder imports a base TDD hat), but introduces circular import risk and requires stack-based cycle detection. Disallowing transitive imports in Phase 1 makes circular imports impossible and keeps the implementation simple — imported files must be self-contained hat definitions.

**Answer: (a) No transitive imports. Imported files must be self-contained.**

---

### Q3: How should the imported hat file format handle the `events:` section?

The issue proposes that imported hat files can include an `events:` section with metadata (description, on_trigger, on_publish) that gets merged into the consuming preset's top-level `events:`. This is convenient but means a single hat file produces side effects beyond just the hat definition itself.

**Answer: (b) No `events:` in imported hat files. Hat files are pure hat definitions only.**

Rationale:
- `events:` is a preset-level concern (how hats relate to each other), not a single-hat concern
- Only 1 of 16 presets uses `events:` today — it's an undocumented, rarely-used feature
- Importing a hat should not produce side effects on the preset's top-level `events:` section
- The consuming preset controls its own event documentation — if you import a builder and want event metadata for `build.done`, add it to your preset's `events:` where it's visible and reviewable
- Easier to add later than to remove — if demand emerges, we can support it in a future iteration

---

### Q4: Where should shared hat files live by convention?

Import paths are relative to the importing file, so any directory works technically. But a convention helps discoverability. The `presets/` directory currently has one subdirectory (`minimal/`). Some options:

**Answer: (c) No convention. Paths are relative to the importing file. Document this clearly and let users organize as they see fit.**

---

### Q5: Should the `import:` key be rejected if it appears in an embedded (builtin) preset?

Embedded presets are compiled into the binary via `include_str!()` — they have no filesystem context, so relative paths can't be resolved. Options:

**Answer: (a) Reject `import:` in embedded presets with a clear error message.**

---

### Q6: What fields should be overridable when importing a hat?

The issue proposes that any HatConfig field can be specified alongside `import:` to override the imported value (field-level replacement, not merge). Should all fields be overridable, or should some be locked?

**Answer: (a) All HatConfig fields are overridable. Maximum flexibility for consumers.**

---

### Q7: Should `import:` resolution happen before or after the `-H` hat overlay merge?

Ralph supports split config: `-c ralph.yml -H builtin:feature`. The hats from `-H` are merged on top of hats from `-c` via `merge_hats_overlay()`. Import resolution could happen:

**Answer: (a) Before overlay merge. Each source resolves its own imports independently.**

Rationale: Relative path resolution requires knowing which file the `import:` came from. After overlay merge, that context is lost. Resolving before merge keeps each file self-contained and avoids coupling merge logic with import logic.

Additionally, imports work in any **file-based** source regardless of flag:

| Source | Allow `import:`? | Reason |
|--------|-----------------|--------|
| `-c ./ralph.yml` (file) | Yes | Has filesystem context |
| `-H ./my-hats.yml` (file) | Yes | Has filesystem context |
| `-H builtin:feature` (embedded) | No | No filesystem context (Q5) |
| `-H https://...` (remote) | No | Phase 1 is local only |

---

### Q8: How should errors be reported when an imported file is invalid?

Failure modes: file not found, invalid YAML, valid YAML but not a valid hat definition, transitive import rejected.

**Answer: (a) Errors point to both the importing file and the imported file, like a stack trace.**

Example:
```
error: failed to resolve hat import
  --> my-workflow.yml, hat 'builder'
  --> imports ./shared/builder.yml

  cause: expected field 'name' to be a string, found sequence
```

The importing file is the "call site" and should always be visible. Essential for debugging as shared hat files accumulate.

---

### Q9: Should the `validate()` step produce warnings for common import pitfalls?

For example: an imported hat's `publishes` references events that no other hat in the preset triggers on (orphaned events), or the imported hat duplicates a hat ID already defined inline.

**Answer: (b) Rely on existing validation. No import-specific warnings.**

Rationale: By the time validation runs, imports are already resolved into regular HatConfig entries — validation doesn't need to know whether a hat came from an import or was inline. Existing validation already catches ambiguous trigger routing, missing completion promises, etc. Keeps import resolution (loading concern) and validation (structural concern) cleanly separated.
