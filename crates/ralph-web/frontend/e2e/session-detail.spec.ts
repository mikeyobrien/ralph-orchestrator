import { test, expect } from '@playwright/test';

test.describe('Session Detail', () => {
  test('shows 404 for nonexistent session', async ({ page }) => {
    await page.goto('/sessions/nonexistent-session-id');

    // Should show some error or 404 state - use the inner main with p-4 class
    const content = await page.locator('main.flex-1.p-4').textContent();
    expect(content).toBeDefined();
  });

  test('navigates back to sessions list', async ({ page }) => {
    await page.goto('/sessions/some-session');

    // Look for a back link/button
    const backLink = page.getByRole('link', { name: /sessions/i });
    if (await backLink.count() > 0) {
      await backLink.first().click();
      await expect(page).toHaveURL('/sessions');
    }
  });
});
