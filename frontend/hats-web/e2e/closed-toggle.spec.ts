import { test, expect } from "@playwright/test";

test.describe("Closed Task Toggle", () => {
  test("should have Show closed button visible", async ({ page }) => {
    await page.goto("http://localhost:5173/tasks");

    // Wait for the page to load
    await page.waitForLoadState("networkidle");

    // Look for the "Show closed" button - use title attribute for precise targeting
    const showClosedButton = page.getByTitle("Show closed tasks");
    await expect(showClosedButton).toBeVisible();
  });

  test("button should toggle between Show/Hide closed", async ({ page }) => {
    await page.goto("http://localhost:5173/tasks");
    await page.waitForLoadState("networkidle");

    // Initially should show "Show closed" - use title for precise targeting
    const toggleButton = page.getByTitle("Show closed tasks");
    await expect(toggleButton).toBeVisible();

    // Click to toggle
    await toggleButton.click();

    // Now should show "Hide closed"
    const hideButton = page.getByTitle("Hide closed tasks");
    await expect(hideButton).toBeVisible();

    // Click again to toggle back
    await hideButton.click();

    // Should be back to "Show closed"
    await expect(page.getByTitle("Show closed tasks")).toBeVisible();
  });
});
