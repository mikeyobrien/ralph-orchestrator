import { test, expect } from '@playwright/test';

test.describe('Navigation', () => {
  test('has navigation menu', async ({ page }) => {
    await page.goto('/');

    // Should have navigation links (sidebar uses ul elements, not nav)
    await expect(page.getByRole('link', { name: /sessions/i })).toBeVisible();
  });

  test('navigates to sessions page', async ({ page }) => {
    await page.goto('/');

    await page.getByRole('link', { name: /sessions/i }).first().click();
    await expect(page).toHaveURL('/sessions');
  });

  test('navigates to live page', async ({ page }) => {
    await page.goto('/');

    await page.getByRole('link', { name: /live/i }).first().click();
    await expect(page).toHaveURL('/live');
  });

  test('navigates to start page', async ({ page }) => {
    await page.goto('/');

    await page.getByRole('link', { name: /start/i }).first().click();
    await expect(page).toHaveURL('/start');
  });

  test('home page loads successfully', async ({ page }) => {
    await page.goto('/');

    // Should load without errors - check for main content area (the inner main with p-4 class)
    const content = await page.locator('main.flex-1.p-4').textContent();
    expect(content).toBeDefined();
  });
});
