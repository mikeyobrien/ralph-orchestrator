/**
 * Builder page end-to-end tests.
 *
 * Covers:
 *  Collection List: top-level heading and primary actions
 *  New Collection + Save/Edit: editor opens, dirty flag, save feedback
 *  Import YAML: dialog open/close, validation, success, error
 *  Run + Observation Mode: RPC invocation, UI transitions, agent-live
 *    indicator, edge-fire animation, Stop exits cleanly
 *  Node/Edge Deletion: Backspace and Delete both remove selected nodes
 *  Role-Based Visual Identity: palette exposes role templates
 *  Blueprints: cards render, clicking creates a pre-populated collection
 *  Run Error Feedback: error banner appears on failure, can be dismissed
 *
 * Conventions:
 *  Tests that need a saved collection use {@link createAndSaveCollection}.
 *  Tests that need `collection.run` to succeed without a real backend use
 *    {@link mockCollectionRunSuccess} to intercept the RPC and return a
 *    canned success payload.
 *  Observation-state tests drive the exposed `window.__observationStore`
 *    directly via {@link driveHatActive} / {@link driveHatPending} /
 *    {@link fireEdgeInStore} instead of mocking the WebSocket. The store
 *    is only attached in dev builds (see `stores/observationStore.ts`);
 *    production bundles are unaffected.
 */

import { expect, test, type Page } from "@playwright/test";

// ─── Timeouts (named so intent is obvious in assertions) ──────────────────
/** Visible duration of the "Saved" badge before it fades back to idle. */
const SAVE_FEEDBACK_MS = 5000;
/** How long the "Saved" status lingers before auto-clearing to idle. */
const SAVE_STATUS_CLEAR_MS = 3500;
/** Wait for the collection.run RPC round-trip to settle. */
const RPC_SETTLE_MS = 2000;
/** Generic short wait for a dialog or popover to render. */
const SHORT_WAIT_MS = 3000;

// ─── Reusable helpers ─────────────────────────────────────────────────────

/**
 * Creates a new collection with the given name and saves it.
 * Leaves the user on the editor page with the Run button enabled.
 */
async function createAndSaveCollection(page: Page, name: string): Promise<void> {
  await page.getByRole("button", { name: "New Collection" }).click();
  const nameInput = page.getByPlaceholder("Collection name");
  await nameInput.clear();
  await nameInput.fill(name);
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Saved")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
}

/**
 * Intercepts all `/rpc/v1` calls and returns a canned success response for
 * `collection.run`. All other methods fall through to the real API.
 */
async function mockCollectionRunSuccess(page: Page): Promise<void> {
  await page.route("**/rpc/v1", async (route) => {
    const body = route.request().postDataJSON();
    if (body?.method === "collection.run") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          apiVersion: "v1",
          id: body.id,
          method: "collection.run",
          meta: { servedAt: new Date().toISOString(), servedBy: "mock" },
          result: { success: true, configPath: "/tmp/mock.yml", pid: 1 },
        }),
      });
      return;
    }
    await route.fallback();
  });
}

/**
 * Intercepts `/rpc/v1` and returns a canned failure for `collection.run`.
 * Used to test the error-banner UI without depending on the local backend
 * being absent or broken.
 */
async function mockCollectionRunFailure(page: Page, message = "ralph run exited with exit code 1:\nbackend not found"): Promise<void> {
  await page.route("**/rpc/v1", async (route) => {
    const body = route.request().postDataJSON();
    if (body?.method === "collection.run") {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          apiVersion: "v1",
          id: body.id,
          method: "collection.run",
          meta: { servedAt: new Date().toISOString(), servedBy: "mock" },
          error: { code: "INTERNAL", message, retryable: false },
        }),
      });
      return;
    }
    await route.fallback();
  });
}

/**
 * Dispatches a synthetic observation transition into the store.
 *
 * The store API is action-based (`setHatActive`, `setHatPending`, etc.);
 * the hook owns the event→action mapping. Tests call actions directly so
 * the assertions aren't coupled to Ralph's event shape.
 */
async function driveHatActive(page: Page, hatId: string): Promise<void> {
  await page.evaluate((id) => {
    const store = (window as unknown as {
      __observationStore: {
        getState: () => { setHatActive: (hatId: string) => void };
      };
    }).__observationStore;
    store.getState().setHatActive(id);
  }, hatId);
}

async function driveHatPending(page: Page, hatId: string): Promise<void> {
  await page.evaluate((id) => {
    const store = (window as unknown as {
      __observationStore: {
        getState: () => { setHatPending: (hatId: string) => void };
      };
    }).__observationStore;
    store.getState().setHatPending(id);
  }, hatId);
}

/** Fires the given edge id in the observation store (drives pulse animation). */
async function fireEdgeInStore(page: Page, edgeId: string): Promise<void> {
  await page.evaluate((id) => {
    const store = (window as unknown as {
      __observationStore: { getState: () => { fireEdge: (id: string) => void } };
    }).__observationStore;
    store.getState().fireEdge(id);
  }, edgeId);
}

// ─── Tests ────────────────────────────────────────────────────────────────

test.describe("Builder Page", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/builder");
    await page.waitForLoadState("networkidle");
  });

  /** Collection list: entry point rendering and primary actions. */
  test.describe("Collection List", () => {
    test("shows the Hat Builder heading", async ({ page }) => {
      await expect(page.getByText("Hat Builder")).toBeVisible();
    });

    test("shows New Collection and Import YAML buttons", async ({ page }) => {
      await expect(page.getByRole("button", { name: "New Collection" })).toBeVisible();
      await expect(page.getByRole("button", { name: "Import YAML" })).toBeVisible();
    });
  });

  /** New Collection: editor opens with the expected shell elements. */
  test.describe("New Collection", () => {
    test("clicking New Collection opens the editor", async ({ page }) => {
      await page.getByRole("button", { name: "New Collection" }).click();

      await expect(page.getByPlaceholder("Collection name")).toBeVisible();
      await expect(page.getByText("Hat Palette")).toBeVisible();
      await expect(page.getByRole("button", { name: "Back" })).toBeVisible();
      await expect(page.getByRole("button", { name: "Save" })).toBeVisible();
    });

    test("Back button returns to list", async ({ page }) => {
      await page.getByRole("button", { name: "New Collection" }).click();
      await expect(page.getByPlaceholder("Collection name")).toBeVisible();

      await page.getByRole("button", { name: "Back" }).click();
      await expect(page.getByText("Hat Builder")).toBeVisible();
    });
  });

  /** Save and Edit flow: dirty indicator, success feedback. */
  test.describe("Save and Edit Flow", () => {
    test("can save a collection and see Saved feedback", async ({ page }) => {
      await createAndSaveCollection(page, "Test Collection");
    });

    test("editing name after save shows Unsaved changes badge", async ({ page }) => {
      await createAndSaveCollection(page, "Test Collection");

      // Wait for the "Saved" status to auto-clear so it doesn't mask the badge.
      await page.waitForTimeout(SAVE_STATUS_CLEAR_MS);

      await page.getByPlaceholder("Collection name").fill("Renamed Collection");
      await expect(page.getByText("Unsaved changes")).toBeVisible();
    });
  });

  /** Import YAML dialog: open/close, validation, happy path, error path. */
  test.describe("Import YAML Dialog", () => {
    test("opens and closes the import dialog", async ({ page }) => {
      await page.getByRole("button", { name: "Import YAML" }).click();

      await expect(page.locator("text=Paste a preset YAML")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await expect(page.getByLabel("Name", { exact: true })).toBeVisible();
      await expect(page.getByRole("button", { name: "Import", exact: true })).toBeVisible();

      await page.getByRole("button", { name: "Cancel" }).click();
      await expect(page.getByText("Hat Builder")).toBeVisible();
    });

    test("Import button is disabled without name and YAML", async ({ page }) => {
      await page.getByRole("button", { name: "Import YAML" }).click();
      await expect(page.locator("text=Paste a preset YAML")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      await expect(page.getByRole("button", { name: "Import", exact: true })).toBeDisabled();
    });

    test("can import a valid YAML and land in editor", async ({ page }) => {
      await page.getByRole("button", { name: "Import YAML" }).click();
      await expect(page.locator("text=Paste a preset YAML")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      await page.getByLabel("Name", { exact: true }).fill("Imported Flow");
      await page.getByLabel("YAML").fill(
        `hats:
  planner:
    name: Planner
    description: Plans work
    triggers: [work.start]
    publishes: [build.task]
  builder:
    name: Builder
    description: Builds things
    triggers: [build.task]
    publishes: [build.done]`
      );
      await page.getByRole("button", { name: "Import", exact: true }).click();

      await expect(page.getByPlaceholder("Collection name")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
    });

    test("shows error for invalid YAML", async ({ page }) => {
      await page.getByRole("button", { name: "Import YAML" }).click();
      await expect(page.locator("text=Paste a preset YAML")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      await page.getByLabel("Name", { exact: true }).fill("Bad Import");
      await page.getByLabel("YAML").fill("not: [valid: yaml: [[[");
      await page.getByRole("button", { name: "Import", exact: true }).click();

      await expect(page.getByRole("alert")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
    });
  });

  /**
   * Run + Observation: the critical path. Verifies the RPC is invoked,
   * observation UI appears, agent-live state and edge animations reflect
   * store updates, and Stop cleanly exits back to edit mode.
   */
  test.describe("Run and Observation Mode", () => {
    test("Run button is visible and enabled after saving a collection", async ({ page }) => {
      await createAndSaveCollection(page, "Runnable Collection");

      const runButton = page.getByRole("button", { name: "Run" });
      await expect(runButton).toBeVisible();
      await expect(runButton).toBeEnabled();
    });

    test("Run button is disabled when collection has unsaved changes", async ({ page }) => {
      await createAndSaveCollection(page, "Dirty Test");

      // Let the "Saved" badge auto-clear before we modify — otherwise the
      // dirty-flag detection can race with the save-status state.
      await page.waitForTimeout(SAVE_STATUS_CLEAR_MS);

      await page.getByPlaceholder("Collection name").fill("Dirty Test Modified");

      const runButton = page.getByRole("button", { name: "Run" });
      await expect(runButton).toBeVisible();
      await expect(runButton).toBeDisabled();
    });

    test("clicking Run opens the prompt dialog and invokes collection.run", async ({ page }) => {
      const rpcMethods: string[] = [];
      page.on("response", async (response) => {
        if (!response.url().includes("/rpc/v1")) return;
        try {
          const body = await response.json();
          rpcMethods.push(body.method ?? "unknown");
        } catch { /* non-JSON response, ignore */ }
      });

      await createAndSaveCollection(page, "Observable Collection");

      const runButton = page.getByRole("button", { name: "Run" });
      await expect(runButton).toBeEnabled();
      await runButton.click();

      await expect(page.getByText("What should")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await expect(page.getByLabel("Prompt")).toBeVisible();

      await page.getByLabel("Prompt").fill("Add input validation");
      await page.getByRole("button", { name: "Run" }).last().click();

      // Always expect the RPC to be attempted. Backend success or failure is
      // orthogonal; we just require the call was issued.
      await expect
        .poll(() => rpcMethods.includes("collection.run"), {
          timeout: SAVE_FEEDBACK_MS,
          message: "expected collection.run RPC to be attempted",
        })
        .toBe(true);
    });

    test("Run success transitions the UI into observation mode", async ({ page }) => {
      await mockCollectionRunSuccess(page);
      await createAndSaveCollection(page, "Mocked Run Collection");

      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("mocked task");
      await page.getByRole("button", { name: "Run" }).last().click();

      await expect(page.getByRole("button", { name: "Stop" })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await expect(page.getByText("Observing live loop execution")).toBeVisible();

      // Edit-only affordances are hidden during observation.
      await expect(page.getByRole("button", { name: "Back" })).toHaveCount(0);
      await expect(page.getByText("Hat Palette")).toHaveCount(0);
    });

    test("observation overlay shows which agent is live and transitions on events", async ({ page }) => {
      await mockCollectionRunSuccess(page);

      // Load the Code Assist blueprint so we have deterministic hats.
      await page.locator("text=⚡").first().click();
      await expect(page.getByPlaceholder("Collection name")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await page.getByRole("button", { name: "Save" }).click();
      await expect(page.getByText("Saved")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("mocked observation");
      await page.getByRole("button", { name: "Run" }).last().click();
      await expect(page.getByRole("button", { name: "Stop" })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      // All hats start idle.
      const nodes = page.locator("[data-hat-key]");
      await expect(nodes.first()).toBeVisible();
      const nodeCount = await nodes.count();
      for (let i = 0; i < nodeCount; i++) {
        await expect(nodes.nth(i)).toHaveAttribute("data-observation-state", "idle");
      }

      // First hat-level event equivalent: planner is emitting (active),
      // builder is its downstream trigger (pending).
      await driveHatActive(page, "planner");
      await driveHatPending(page, "builder");

      await expect(page.locator('[data-hat-key="planner"]'))
        .toHaveAttribute("data-observation-state", "active", { timeout: RPC_SETTLE_MS });
      await expect(page.locator('[data-hat-key="builder"]'))
        .toHaveAttribute("data-observation-state", "pending");

      // Second transition: builder takes over (previous active → completed),
      // reviewer becomes pending.
      await driveHatActive(page, "builder");
      await driveHatPending(page, "reviewer");

      await expect(page.locator('[data-hat-key="planner"]'))
        .toHaveAttribute("data-observation-state", "completed");
      await expect(page.locator('[data-hat-key="builder"]'))
        .toHaveAttribute("data-observation-state", "active");
      await expect(page.locator('[data-hat-key="reviewer"]'))
        .toHaveAttribute("data-observation-state", "pending");
    });

    test("edge fire attribute toggles via store during observation", async ({ page }) => {
      await mockCollectionRunSuccess(page);

      await page.locator("text=⚡").first().click();
      await expect(page.getByPlaceholder("Collection name")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await page.getByRole("button", { name: "Save" }).click();
      await expect(page.getByText("Saved")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("mocked fire");
      await page.getByRole("button", { name: "Run" }).last().click();
      await expect(page.getByRole("button", { name: "Stop" })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      // Edges only get data-edge-id once the auto-layout has run.
      const firstEdge = page.locator("[data-edge-id]").first();
      await expect(firstEdge).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      const firstEdgeId = await firstEdge.getAttribute("data-edge-id");
      expect(firstEdgeId).toBeTruthy();

      await expect(firstEdge).toHaveAttribute("data-fired", "false");

      await fireEdgeInStore(page, firstEdgeId!);

      await expect(page.locator(`[data-edge-id="${firstEdgeId}"]`))
        .toHaveAttribute("data-fired", "true", { timeout: 1000 });
    });

    test("Stop exits observation mode and restores edit affordances", async ({ page }) => {
      await mockCollectionRunSuccess(page);
      await createAndSaveCollection(page, "Stoppable Collection");

      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("mocked task");
      await page.getByRole("button", { name: "Run" }).last().click();

      const stopButton = page.getByRole("button", { name: "Stop" });
      await expect(stopButton).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      await stopButton.click();

      await expect(page.getByRole("button", { name: "Run" })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await expect(page.getByRole("button", { name: "Back" })).toBeVisible();
      await expect(page.getByText("Hat Palette")).toBeVisible();
    });

    test("Properties panel is hidden during observation", async ({ page }) => {
      await mockCollectionRunSuccess(page);
      await createAndSaveCollection(page, "Properties Hidden Test");

      // Before running: properties panel visible on the right.
      await expect(page.getByText("Properties", { exact: true })).toBeVisible();

      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("mocked task");
      await page.getByRole("button", { name: "Run" }).last().click();

      await expect(page.getByRole("button", { name: "Stop" })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      // During observation: properties panel is hidden.
      await expect(page.getByText("Properties", { exact: true })).toHaveCount(0);

      // Stop brings the properties panel back.
      await page.getByRole("button", { name: "Stop" }).click();
      await expect(page.getByText("Properties", { exact: true })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
    });

    test("completeAll terminal transition marks every node completed", async ({ page }) => {
      await mockCollectionRunSuccess(page);

      await page.locator("text=⚡").first().click();
      await expect(page.getByPlaceholder("Collection name")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await page.getByRole("button", { name: "Save" }).click();
      await expect(page.getByText("Saved")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("mocked terminal");
      await page.getByRole("button", { name: "Run" }).last().click();
      await expect(page.getByRole("button", { name: "Stop" })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      // Seed with a couple of hat states so completeAll has something to transition.
      await driveHatActive(page, "planner");
      await driveHatPending(page, "builder");

      // Trigger the terminal transition via the store.
      await page.evaluate(() => {
        const store = (window as unknown as {
          __observationStore: { getState: () => { completeAll: () => void } };
        }).__observationStore;
        store.getState().completeAll();
      });

      // Both touched nodes end up completed. (completeAll leaves nodes
      // that were never seen in their default "idle" — only nodes that
      // already appeared in nodeStates are marked completed.)
      await expect(page.locator('[data-hat-key="planner"]'))
        .toHaveAttribute("data-observation-state", "completed", { timeout: RPC_SETTLE_MS });
      await expect(page.locator('[data-hat-key="builder"]'))
        .toHaveAttribute("data-observation-state", "completed");
    });

    test("full 4-hat chain: every node transitions idle -> active -> completed in order", async ({ page }) => {
      await mockCollectionRunSuccess(page);

      // Load the Code Assist blueprint (4 hats: planner, builder, reviewer, finalizer).
      await page.locator("text=⚡").first().click();
      await expect(page.getByPlaceholder("Collection name")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await page.getByRole("button", { name: "Save" }).click();
      await expect(page.getByText("Saved")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      // Enter observation mode.
      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("full chain test");
      await page.getByRole("button", { name: "Run" }).last().click();
      await expect(page.getByRole("button", { name: "Stop" })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      // All nodes start idle.
      for (const hat of ["planner", "builder", "reviewer", "finalizer"]) {
        await expect(page.locator(`[data-hat-key="${hat}"]`))
          .toHaveAttribute("data-observation-state", "idle");
      }

      // Step 1: planner active.
      await driveHatActive(page, "planner");
      await expect(page.locator('[data-hat-key="planner"]'))
        .toHaveAttribute("data-observation-state", "active", { timeout: RPC_SETTLE_MS });

      // Step 2: builder active, planner completed.
      await driveHatActive(page, "builder");
      await expect(page.locator('[data-hat-key="planner"]'))
        .toHaveAttribute("data-observation-state", "completed");
      await expect(page.locator('[data-hat-key="builder"]'))
        .toHaveAttribute("data-observation-state", "active");

      // Step 3: reviewer active, builder completed.
      await driveHatActive(page, "reviewer");
      await expect(page.locator('[data-hat-key="builder"]'))
        .toHaveAttribute("data-observation-state", "completed");
      await expect(page.locator('[data-hat-key="reviewer"]'))
        .toHaveAttribute("data-observation-state", "active");

      // Step 4: finalizer active, reviewer completed.
      await driveHatActive(page, "finalizer");
      await expect(page.locator('[data-hat-key="reviewer"]'))
        .toHaveAttribute("data-observation-state", "completed");
      await expect(page.locator('[data-hat-key="finalizer"]'))
        .toHaveAttribute("data-observation-state", "active");

      // Terminal: all completed.
      await page.evaluate(() => {
        const store = (window as unknown as {
          __observationStore: { getState: () => { completeAll: () => void } };
        }).__observationStore;
        store.getState().completeAll();
      });

      for (const hat of ["planner", "builder", "reviewer", "finalizer"]) {
        await expect(page.locator(`[data-hat-key="${hat}"]`))
          .toHaveAttribute("data-observation-state", "completed", { timeout: RPC_SETTLE_MS });
      }
    });

    test("observation legend is visible with colored dots during observation", async ({ page }) => {
      await mockCollectionRunSuccess(page);
      await createAndSaveCollection(page, "Legend Test");

      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("legend check");
      await page.getByRole("button", { name: "Run" }).last().click();
      await expect(page.getByRole("button", { name: "Stop" })).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      // Legend should show three labeled states.
      await expect(page.getByText("pending")).toBeVisible();
      await expect(page.getByText("active")).toBeVisible();
      await expect(page.getByText("done")).toBeVisible();
    });
  });

  /**
   * Edge and Node Deletion: Backspace and Delete are both supported (Mac
   * keyboards often don't have a standalone Delete key).
   */
  test.describe("Edge and Node Deletion", () => {
    for (const key of ["Backspace", "Delete"] as const) {
      test(`${key} removes a selected node`, async ({ page }) => {
        // Load a blueprint so the canvas has nodes without drag-drop.
        await page.locator("text=⚡").first().click();
        await expect(page.getByPlaceholder("Collection name")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

        const nodes = page.locator("[data-hat-key]");
        await expect(nodes.first()).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
        const initialCount = await nodes.count();
        expect(initialCount).toBeGreaterThan(0);

        await nodes.first().click();
        await page.keyboard.press(key);

        await expect(nodes).toHaveCount(initialCount - 1, { timeout: SHORT_WAIT_MS });
      });
    }
  });

  /** Role-Based Visual Identity: palette exposes the role templates. */
  test.describe("Role-Based Visual Identity", () => {
    test("palette shows hat templates with role names", async ({ page }) => {
      await page.getByRole("button", { name: "New Collection" }).click();

      await expect(page.getByText("Planner").first()).toBeVisible();
      await expect(page.getByText("Builder").first()).toBeVisible();
      await expect(page.getByText("Reviewer").first()).toBeVisible();
      await expect(page.getByText("Validator").first()).toBeVisible();
      await expect(page.getByText("Confessor").first()).toBeVisible();
      await expect(page.getByText("Custom Hat").first()).toBeVisible();
    });
  });

  /** Blueprints: cards render on list page and materialize into collections. */
  test.describe("Blueprints", () => {
    test("shows blueprint template cards on the list page", async ({ page }) => {
      await expect(page.getByText("Start from a blueprint")).toBeVisible();
      // Use exact match because user-written collection descriptions may
      // contain strings like "4 hats" and shadow the blueprint badges.
      await expect(page.getByText("4 hats", { exact: true })).toBeVisible();
      await expect(page.getByText("3 hats", { exact: true })).toBeVisible();
      await expect(page.getByText("2 hats", { exact: true })).toBeVisible();
    });

    test("clicking the Debug blueprint creates a collection with 3 hats", async ({ page }) => {
      await page.locator("text=🔍").first().click();
      await expect(page.getByPlaceholder("Collection name")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      const nodes = page.locator("[data-hat-key]");
      await expect(nodes.first()).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await expect(nodes).toHaveCount(3);
    });
  });

  /** Run Error Feedback: failure surfaces a dismissable banner. */
  test.describe("Run Error Feedback", () => {
    test("shows error banner when collection.run fails", async ({ page }) => {
      await mockCollectionRunFailure(page);
      await createAndSaveCollection(page, "Error Test");

      await page.getByRole("button", { name: "Run" }).click();
      await expect(page.getByLabel("Prompt")).toBeVisible({ timeout: SHORT_WAIT_MS });
      await page.getByLabel("Prompt").fill("Test prompt");
      await page.getByRole("button", { name: "Run" }).last().click();

      // Error banner shows ralph's own stderr. We anchor on the Dismiss
      // affordance since the exact message depends on the mock payload.
      await expect(page.getByText("Dismiss")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });

      await page.getByText("Dismiss").click();
      await expect(page.getByText("Dismiss")).not.toBeVisible();
    });

    test("Run button remains enabled after error dismissal", async ({ page }) => {
      await mockCollectionRunFailure(page);
      await createAndSaveCollection(page, "Retry Test");

      await page.getByRole("button", { name: "Run" }).click();
      await page.getByLabel("Prompt").fill("Test");
      await page.getByRole("button", { name: "Run" }).last().click();

      await expect(page.getByText("Dismiss")).toBeVisible({ timeout: SAVE_FEEDBACK_MS });
      await page.getByText("Dismiss").click();

      await expect(page.getByRole("button", { name: "Run" })).toBeEnabled();
    });
  });
});
