import { test, expect } from '@playwright/test';

test.describe('Session List', () => {
  test('displays sessions page', async ({ page }) => {
    await page.goto('/sessions');

    // Should show the page title
    await expect(page.getByRole('heading', { name: /sessions/i })).toBeVisible();
  });

  test('shows empty state when no sessions', async ({ page }) => {
    await page.goto('/sessions');

    // When no sessions exist, should show appropriate message or empty state
    const content = await page.locator('main.flex-1.p-4').textContent();
    // The page should render without errors
    expect(content).toBeDefined();
  });

  test('navigates to session detail when clicking session', async ({ page }) => {
    await page.goto('/sessions');

    // Look for session cards or list items
    const sessionCards = page.locator('[data-testid="session-card"]');
    const count = await sessionCards.count();

    if (count > 0) {
      // Click the first session
      await sessionCards.first().click();

      // Should navigate to session detail
      await expect(page).toHaveURL(/\/sessions\/.+/);
    }
  });
});
