# Task: User-Collaborative Validation Gate System

Build an opt-in validation feature that enables Ralph Orchestrator to propose functional validation strategies to users. The system analyzes projects and PROPOSES validation approaches - it does NOT auto-generate configurations. Users must confirm before validation proceeds.

## The Problem

Current Ralph Orchestrator lacks end-user validation beyond build/test:
- No functional validation (browser, simulator, CLI output)
- No framework-agnostic validation gate architecture
- If validation were added naively, it would be:
  - Auto-configured (bypassing user involvement)
  - Hardcoded to specific MCP servers (Puppeteer, Playwright, xc-mcp)
  - Always-on (not opt-in)

## Objective

Create a validation system that:
1. **Is opt-in** - Disabled by default (`enable_validation=False`)
2. **Is Claude-only** - Only works with Claude adapter (for now)
3. **Is collaborative** - AI proposes, user confirms
4. **Is flexible** - No hardcoded tools; AI recommends based on project context
5. **Is transparent** - User sees and approves validation strategy before work begins
6. **Inherits user's tools** - Leverages user's Claude Code MCP servers and settings

---

## Philosophy: Use Their Tools to Validate Their Projects

The validation system should **leverage the user's existing Claude Code ecosystem**. When running validation with `inherit_user_settings=True`, Ralph inherits:

### Inherit User's MCP Servers
The user's `~/.claude/settings.json` MCP servers become available. If they have:
- **mcp-puppeteer** / **mcp-playwright** → Use for browser automation
- **xc-mcp** / **ios-simulator** → Use for iOS simulator validation
- **mcp-filesystem** → Access project files for validation scripts
- **Custom MCPs** → Propose using whatever tools the user has available

The ClaudeAdapter supports `inherit_user_settings=True` which loads `setting_sources: ['user', 'project', 'local']`. The validation proposal phase runs Claude with these settings active.

### Tool Discovery During Proposal
During the proposal phase, Claude should:
1. Check what MCP servers are available (via tool listing)
2. Propose validation strategies that USE those available tools
3. If no automation tools exist, suggest manual validation or fallback options
4. NEVER assume specific MCPs - discover and adapt

```python
# Example: Validation proposal with inherited settings
adapter = ClaudeAdapter(inherit_user_settings=True)
response = await adapter.aexecute(
    proposal_prompt,
    inherit_user_settings=True,  # Load user's ~/.claude/settings.json
    enable_all_tools=True,       # Access to user's MCP servers
)
```

---

## Philosophy: Propose, Don't Prescribe

The validation system should **never auto-generate configurations**. Instead:

### Session 0: Proposal Phase
When validation is enabled, before any implementation work:
1. AI analyzes the project (type, build commands, run commands, test framework)
2. AI drafts a validation proposal
3. AI presents proposal to user conversationally
4. User confirms, modifies, or declines
5. Only after confirmation does validation proceed

### User-Centric Design
```
┌─────────────────────────────────────────────────────────────┐
│                    WRONG APPROACH                           │
├─────────────────────────────────────────────────────────────┤
│  1. Detect project type                                     │
│  2. Auto-generate validation_config.json                    │
│  3. Hardcode: "if web → use Puppeteer"                     │
│  4. Run validation without asking                           │
│                                                             │
│  Problems: Prescriptive, inflexible, ignores user context   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                    RIGHT APPROACH                           │
├─────────────────────────────────────────────────────────────┤
│  1. Detect project type                                     │
│  2. Draft a validation PROPOSAL                             │
│  3. Present to user: "Here's what I recommend..."           │
│  4. Ask: "Does this make sense? Approve/Modify/Skip?"       │
│  5. Only proceed after explicit user confirmation           │
│                                                             │
│  Benefits: Collaborative, flexible, user maintains control  │
└─────────────────────────────────────────────────────────────┘
```

---

## Architecture

### Orchestrator Changes

Modify `src/ralph_orchestrator/orchestrator.py`:

```python
class RalphOrchestrator:
    def __init__(
        self,
        # ... existing params ...
        enable_validation: bool = False,      # NEW: opt-in flag
        validation_interactive: bool = True,  # NEW: require user confirmation
    ):
        # Guard: validation only for Claude
        if enable_validation and self.primary_tool != "claude":
            raise ValueError(
                "Validation feature is only available with Claude adapter. "
                f"Current adapter: {self.primary_tool}"
            )

        self.enable_validation = enable_validation
        self.validation_interactive = validation_interactive
        self.validation_proposal = None   # Stores AI's proposed strategy
        self.validation_approved = False  # User confirmation status
```

### Validation Proposal Flow

```
┌─────────────────────────────────────────────────────────────┐
│  arun() Entry Point                                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  if self.enable_validation:                                 │
│      await self._propose_validation_strategy()              │
│                                                             │
│      if not self.validation_approved:                       │
│          logger.info("Validation declined, proceeding")     │
│          self.enable_validation = False                     │
│                                                             │
│  # Continue with normal orchestration...                    │
│  await self._run_main_loop()                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### New Methods

```python
async def _propose_validation_strategy(self):
    """AI proposes validation, user confirms."""

    # Load the proposal prompt
    proposal_prompt = self._load_proposal_prompt()

    # Execute proposal phase - AI analyzes and proposes
    response = await self.current_adapter.execute(
        proposal_prompt,
        context=self._get_project_context()
    )

    # Store proposal for user review
    self.validation_proposal = response

    # If interactive mode, wait for user confirmation
    if self.validation_interactive:
        self.validation_approved = await self._get_user_confirmation()
    else:
        # Non-interactive: auto-approve (for CI/CD scenarios)
        self.validation_approved = True

def _load_proposal_prompt(self) -> str:
    """Load the validation proposal prompt."""
    prompt_path = Path(__file__).parent.parent.parent / "prompts" / "VALIDATION_PROPOSAL_PROMPT.md"
    return prompt_path.read_text()

async def _get_user_confirmation(self) -> bool:
    """Present proposal and get user confirmation."""
    # Implementation depends on UI context (CLI, web, etc.)
    pass
```

---

## Prompt Structure

### VALIDATION_PROPOSAL_PROMPT.md

Create `prompts/VALIDATION_PROPOSAL_PROMPT.md`:

```markdown
# Validation Strategy Proposal

**SESSION 0 - PROPOSAL PHASE (requires user approval)**

## Objective
Analyze the project and PROPOSE (not auto-configure) a validation strategy.
Present your proposal to the user for approval before proceeding.

## Important Principles
1. **Propose, don't prescribe** - Present recommendations, don't auto-generate
2. **User decides** - The user confirms or modifies the approach
3. **Be flexible** - Don't assume specific tools/MCPs are available
4. **Ask questions** - Clarify what the user wants to validate

## Your Task

### Step 1: Analyze the Project
Examine the project structure to understand:
- What type of project is this? (web, iOS, CLI, API, library)
- How is it built? (build commands, dependencies)
- How does a user interact with it? (browser, simulator, command line)
- Are there existing tests? (test frameworks, test commands)

### Step 2: Draft a Validation Proposal
Based on analysis, draft a proposal including:
- What you found about the project
- How you recommend validating it from an end-user perspective
- What tools or methods you would use
- What you need to know from the user

### Step 3: Present to User for Confirmation
Present your proposal conversationally and ask for confirmation.

## Output Requirements
Your output must be a **conversation with the user**, NOT a configuration file.

- DO ask for explicit user confirmation
- DO offer alternatives if the user disagrees
- DO explain WHY you recommend a certain approach
- DO list what questions you have for the user
- DO NOT generate validation_config.json until user confirms
- DO NOT assume specific MCP servers are available
- DO NOT proceed with validation without user approval
```

---

## CLI Integration

### New Flags

```bash
# Enable validation (opt-in)
ralph run -P PROMPT.md --enable-validation

# Disable interactive mode (for CI/CD)
ralph run -P PROMPT.md --enable-validation --no-validation-interactive

# Equivalent using config file
ralph run -P PROMPT.md -c ralph.yml
```

### ralph.yml Config

```yaml
# Validation feature (opt-in)
enable_validation: true
validation_interactive: true  # Set false for CI/CD
```

---

## Success Criteria

### Core Functionality
- [ ] `enable_validation` parameter added to RalphOrchestrator.__init__()
- [ ] Default value is `False` (opt-in behavior)
- [ ] `validation_interactive` parameter added with default `True`
- [ ] ValueError raised when `enable_validation=True` with non-Claude adapter
- [ ] `validation_proposal` attribute exists (None until populated)
- [ ] `validation_approved` attribute exists (False until user confirms)

### Proposal Flow
- [ ] `_propose_validation_strategy()` method implemented
- [ ] `_load_proposal_prompt()` method implemented
- [ ] `_get_user_confirmation()` method implemented
- [ ] Proposal phase executes before main orchestration loop
- [ ] When user declines, `enable_validation` is set to False gracefully

### Prompt
- [ ] `VALIDATION_PROPOSAL_PROMPT.md` exists in prompts/
- [ ] Prompt asks for user confirmation (contains "confirm")
- [ ] Prompt uses "propose" language (contains "propose")
- [ ] Prompt mentions user approval (contains "user approval")
- [ ] Prompt has "do not" instructions (collaborative, not prescriptive)

### Testing
- [ ] Test: validation disabled by default
- [ ] Test: validation can be enabled
- [ ] Test: validation with Claude succeeds
- [ ] Test: validation with Gemini raises ValueError
- [ ] Test: validation with QChat raises ValueError
- [ ] Test: validation_interactive defaults to True
- [ ] Test: proposal prompt loads correctly
- [ ] Test: proposal flow integrates with arun()

### Documentation
- [ ] Update CLI help text with new flags
- [ ] Update docs/guide/ with validation feature
- [ ] Add example usage scenarios

---

## Implementation Phases

### Phase 1: Orchestrator Parameters (Priority: HIGH)
- [ ] Add `enable_validation` parameter to `__init__`
- [ ] Add `validation_interactive` parameter to `__init__`
- [ ] Implement Claude-only guard (raise ValueError for others)
- [ ] Add `validation_proposal` and `validation_approved` attributes
- [ ] Write tests for parameter behavior

### Phase 2: Proposal Methods (Priority: HIGH)
- [ ] Implement `_load_proposal_prompt()` method
- [ ] Implement `_propose_validation_strategy()` method
- [ ] Implement `_get_user_confirmation()` method
- [ ] Write tests for method existence and behavior

### Phase 3: Proposal Prompt (Priority: HIGH)
- [ ] Create `prompts/VALIDATION_PROPOSAL_PROMPT.md`
- [ ] Follow collaborative, user-centric language
- [ ] Include examples for different project types
- [ ] Write tests to validate prompt content

### Phase 4: Orchestration Integration (Priority: HIGH)
- [ ] Integrate proposal phase into `arun()` method
- [ ] Handle user confirmation/declination flow
- [ ] Ensure graceful fallback when declined
- [ ] Write integration tests

### Phase 5: CLI and Config (Priority: MEDIUM)
- [ ] Add `--enable-validation` CLI flag
- [ ] Add `--no-validation-interactive` CLI flag
- [ ] Support config file options
- [ ] Update argument parser

### Phase 6: Documentation (Priority: MEDIUM)
- [ ] Document feature in docs/guide/
- [ ] Add usage examples
- [ ] Update README if needed

---

## Example Scenarios

### Scenario 1: Web Application

```bash
$ ralph run -P PROMPT.md --enable-validation

🔍 Analyzing project for validation strategy...

📋 Validation Proposal:

Based on analyzing your project, here's what I found:

**Project Analysis:**
- Type: Next.js web application
- Build: `npm run build`
- Run: `npm run dev` (serves at localhost:3000)
- Tests: Jest + Playwright tests exist

**My Validation Proposal:**

Since this is a web app, I recommend validating it the way a user would -
by actually loading it in a browser and interacting with it.

My approach:
1. Build the project to catch compilation errors
2. Start the dev server
3. Use browser automation to:
   - Navigate to the main page
   - Verify it renders correctly
   - Test key user interactions

**Questions for you:**
- Which pages or features are most critical to validate?
- Any specific user flows I should test (login, checkout, etc.)?
- Do you have a preferred browser automation tool?

Does this make sense? [Approve/Modify/Skip]: _
```

### Scenario 2: iOS Application

```bash
$ ralph run -P PROMPT.md --enable-validation

🔍 Analyzing project for validation strategy...

📋 Validation Proposal:

**Project Analysis:**
- Type: iOS SwiftUI application
- Build: `xcodebuild -scheme MyApp`
- Run: Requires iOS Simulator
- Tests: XCTest unit tests exist

**My Validation Proposal:**

For iOS apps, I recommend validating by running the app in the Simulator.

My approach:
1. Build the app with xcodebuild
2. Boot an iOS Simulator
3. Install and launch the app
4. Take screenshots to verify UI renders correctly
5. Test key tap interactions if needed

**Questions for you:**
- Which Simulator device should I use? (iPhone 15 Pro, etc.)
- What are the most important screens to validate?
- Any specific flows or features to test?

Does this approach work for you? [Approve/Modify/Skip]: _
```

### Scenario 3: CLI Tool

```bash
$ ralph run -P PROMPT.md --enable-validation

🔍 Analyzing project for validation strategy...

📋 Validation Proposal:

**Project Analysis:**
- Type: Python CLI tool
- Build: `pip install -e .`
- Run: `mycli [command]`
- Tests: pytest tests exist

**My Validation Proposal:**

For CLI tools, I recommend validating by running commands and checking outputs.

My approach:
1. Install the tool locally
2. Run key commands with test inputs
3. Verify exit codes and outputs
4. Check that output files are created correctly

**Questions for you:**
- What are the most important commands to test?
- Any specific inputs or scenarios I should try?
- What outputs indicate success?

Would you like me to proceed with this? [Approve/Modify/Skip]: _
```

---

## Files to Modify

1. `src/ralph_orchestrator/orchestrator.py` - Add parameters, guards, methods
2. `src/ralph_orchestrator/__main__.py` - Add CLI flags
3. `prompts/VALIDATION_PROPOSAL_PROMPT.md` - Create new file
4. `tests/test_validation_feature.py` - Create test suite
5. `docs/guide/validation.md` - Create documentation

## Files to Reference (Existing)

1. `src/ralph_orchestrator/validation/config.py` - Existing validation config (may be useful)
2. `src/ralph_orchestrator/validation/base.py` - Gate abstractions
3. `prompts/VALIDATION_CODING_PROMPT.md` - Used after approval (existing)
4. `prompts/DIAGNOSTIC_PROMPT.md` - May be deprecated/repurposed

---

## Key Differences from Any Prior Implementation

| Aspect | Wrong Approach | Right Approach |
|--------|----------------|----------------|
| Trigger | Always on | Only if `enable_validation=True` |
| Adapter | Any adapter | Claude only |
| MCP selection | Hardcoded mapping | AI proposes flexibly |
| User involvement | None | Confirmation required |
| Config generation | Auto | Only after approval |
| Fallback | None | Graceful disable if declined |

---

## Notes

### Graceful Degradation
- If user declines validation: proceed normally without it
- If proposal fails to generate: log warning, proceed without validation
- If confirmation times out: treat as decline (in interactive mode)

### Future Extensions
- Support for other adapters when they gain equivalent capabilities
- Validation result persistence and reporting
- Integration with CI/CD validation pipelines

### Testing Considerations
- Mock the adapter for unit tests
- Test both interactive and non-interactive modes
- Test the error path (non-Claude adapter)

---

**Status**: 🚧 NOT STARTED
**Priority**: HIGH
**Estimated Effort**: 2-3 development iterations
