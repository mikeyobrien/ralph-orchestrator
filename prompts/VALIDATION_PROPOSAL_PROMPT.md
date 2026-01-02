# Validation Strategy Proposal

**SESSION 0 - PROPOSAL PHASE (requires user approval)**

This prompt guides AI to analyze a project and PROPOSE a validation strategy for user confirmation. The AI does not auto-generate configuration - it presents a proposal and waits for user approval.

## Objective

Analyze the current project and PROPOSE (not auto-configure) a validation strategy. Present your proposal to the user for confirmation before proceeding.

## Important Principles

1. **Propose, don't prescribe** - Present recommendations, don't auto-generate configs
2. **User decides** - The user confirms or modifies the validation approach
3. **Be flexible** - Don't assume specific tools/MCPs are available
4. **Ask questions** - Clarify what the user wants to validate

## Your Task

### Step 1: Analyze the Project

Examine the project structure to understand:
- What type of project is this? (web app, iOS app, CLI tool, API, library, hybrid)
- How is it built? (build commands, dependencies)
- How does a user interact with it? (browser, simulator, command line, API calls)
- Are there existing tests? (test frameworks, test commands)

### Step 2: Draft a Validation Proposal

Based on your analysis, draft a proposal that includes:
- What you found about the project
- How you recommend validating it from an end-user perspective
- What tools or methods you would use
- What you need to know from the user

### Step 3: Present to User for Confirmation

Present your proposal conversationally and ask for confirmation:

```
Based on analyzing your project, here's what I found and recommend:

**Project Analysis:**
- Type: [detected type]
- Build: [build command]
- Run: [how to run/start it]

**My Validation Proposal:**

I recommend validating this project by [approach]. This means:

1. [First validation step]
2. [Second validation step]
3. [Third validation step]

**What I would use:**
- [Tool/method 1]
- [Tool/method 2]

**Questions for you:**
- [Question about what's most important to validate]
- [Question about specific features/flows]

Does this approach make sense for your project? Would you like to:
- Approve this validation strategy
- Modify it (tell me what you'd change)
- Skip validation and proceed without it
```

## Output Requirements

Your output must be a **conversation with the user**, NOT a configuration file.

- DO ask for explicit user confirmation
- DO offer alternatives if the user disagrees
- DO explain WHY you recommend a certain approach
- DO list what questions you have for the user
- DO NOT generate validation_config.json until user confirms
- DO NOT assume specific MCP servers are available
- DO NOT proceed with validation without user approval

## Example Proposals

### Web Application Example

```
I've analyzed your project. Here's what I found:

**Project Analysis:**
- Type: Next.js web application
- Build: `npm run build`
- Run: `npm run dev` (serves at localhost:3000)
- Tests: Jest + Playwright tests exist

**My Validation Proposal:**

Since this is a web app, I recommend validating it the way a user would - by actually loading it in a browser and interacting with it.

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

Does this make sense? Should I proceed with this approach?
```

### iOS App Example

```
I've analyzed your project. Here's what I found:

**Project Analysis:**
- Type: iOS SwiftUI application
- Build: `xcodebuild -scheme MyApp`
- Run: Requires iOS Simulator
- Tests: XCTest unit tests exist

**My Validation Proposal:**

For iOS apps, I recommend validating by actually running the app in the Simulator - this is how a real user would experience it.

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

Does this approach work for you?
```

### CLI Tool Example

```
I've analyzed your project. Here's what I found:

**Project Analysis:**
- Type: Python CLI tool
- Build: `pip install -e .`
- Run: `mycli [command]`
- Tests: pytest tests exist

**My Validation Proposal:**

For CLI tools, I recommend validating by actually running commands and checking outputs - exactly like a user would.

My approach:
1. Install the tool locally
2. Run key commands with test inputs
3. Verify exit codes and outputs
4. Check that output files are created correctly

**Questions for you:**
- What are the most important commands to test?
- Any specific inputs or scenarios I should try?
- What outputs indicate success?

Would you like me to proceed with this?
```

## After User Confirms

Only after the user confirms should you:
1. Generate `validation_config.json` based on the approved approach
2. Proceed with the implementation work

If the user declines validation, simply proceed with the regular implementation without functional validation.

---

**Status**: Ready for execution
**Requires**: User confirmation before proceeding
