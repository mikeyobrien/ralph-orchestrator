---
name: playwriter
description: Browser automation via Playwriter (remorses) using persistent Chrome sessions and the full Playwright Page API.
hats: [developer, qa_tester]
---

# Playwriter

## Overview

Use Playwriter to run Playwright Page scripts against your local Chrome session. This preserves logins, cookies, and extensions, which is ideal for web dashboard testing and authenticated flows.

## When to Use

- Validating the Ralph web dashboard UI
- Navigating authenticated pages without re-logging in
- Testing flows that require browser extensions or saved state
- Capturing accessibility snapshots for element discovery

## Prerequisites

- Install the CLI:
  ```bash
  npm i -g playwriter
  ```
- Install the Playwriter Chrome extension (see the Playwriter repo instructions)
- Ensure Chrome is running and the extension is enabled

## Core Workflow

1. Create a session:
   ```bash
   playwriter session new
   ```
2. List sessions and copy the session id:
   ```bash
   playwriter session list
   ```
3. Execute Playwright code against that session:
   ```bash
   playwriter -s <session_id> -e "await page.goto('https://example.com')"
   ```

## Execution Environment

Within `-e`, these are available in scope:
- `page` (Playwright Page)
- `context` (BrowserContext)
- `state` (persistent object across calls in the same session)
- `require` (for loading helper modules)

Example state persistence:
```bash
playwriter -s <session_id> -e "state.lastUrl = page.url()"
playwriter -s <session_id> -e "console.log(state.lastUrl)"
```

## Common Patterns

### Navigate + click
```bash
playwriter -s <session_id> -e "await page.goto('http://localhost:3000'); await page.getByRole('button', { name: 'Run' }).click();"
```

### Fill forms
```bash
playwriter -s <session_id> -e "await page.getByLabel('Email').fill('qa@example.com'); await page.getByLabel('Password').fill('secret'); await page.getByRole('button', { name: 'Sign in' }).click();"
```

### Accessibility snapshots (labeled)
```bash
playwriter -s <session_id> -e "const { screenshotWithAccessibilityLabels } = require('playwriter'); await screenshotWithAccessibilityLabels(page, { path: '/tmp/a11y.png' });"
```

### Network interception
```bash
playwriter -s <session_id> -e "await page.route('**/api/**', async route => { const res = await route.fetch(); const body = await res.json(); await route.fulfill({ json: { ...body, injected: true } }); });"
```

### Read page content
```bash
playwriter -s <session_id> -e "const text = await page.locator('main').innerText(); console.log(text);"
```

## Tips

- Prefer `getByRole` and `getByLabel` for stable selectors.
- Use accessibility snapshots to discover reliable roles and labels.
- Keep sessions focused: reset or close them if state becomes messy.
- For multi-step flows, store intermediate data on `state`.
