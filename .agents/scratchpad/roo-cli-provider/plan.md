# Plan: Add roo-cli as a Provider

## Test Strategy
All tests in `cli_backend.rs` and `auto_detect.rs` test modules, following existing patterns.

### cli_backend.rs tests
| Test | Validates |
|------|-----------|
| `test_roo_backend` | `roo()` → `roo --print --ephemeral --prompt-file <path> ` |
| `test_roo_interactive` | `roo_interactive()` → `roo "prompt"` |
| `test_from_name_roo` | `from_name("roo")` resolves correctly |
| `test_from_config_roo` | Config `backend: "roo"` creates correct backend |
| `test_from_config_roo_with_args` | Extra args append correctly |
| `test_for_interactive_prompt_roo` | Interactive factory returns `roo_interactive()` |
| `test_roo_interactive_mode_removes_print` | `build_command(_, true)` removes `--print` and `--ephemeral` |
| `test_roo_uses_prompt_file` | All prompts use `--prompt-file` |
| `test_env_vars_default_empty` | Add `CliBackend::roo()` assertion |

### auto_detect.rs tests
| Test | Validates |
|------|-----------|
| `test_detection_command_roo` | Returns `"roo"` |
| `test_default_priority_includes_roo` | "roo" in `DEFAULT_PRIORITY` |
| `test_default_priority_roo_is_last` | "roo" is last in priority |
| `test_no_backend_error_display` | Includes "Roo CLI" |

## Implementation Steps
1. Add `roo()` and `roo_interactive()` constructors
2. Add match arms in `from_config`, `from_name`, `for_interactive_prompt`, `filter_args_for_interactive`
3. Add roo-specific `--prompt-file` handling in `build_command()`
4. Update `auto_detect.rs`: `DEFAULT_PRIORITY`, `NoBackendError`
5. Create `presets/minimal/roo.yml`
6. Update `lib.rs` doc comment
