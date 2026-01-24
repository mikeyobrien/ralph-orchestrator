import { test, expect } from '@playwright/test';

test.describe('Live Monitoring', () => {
  test('displays live monitoring page', async ({ page }) => {
    await page.goto('/live');

    // Should show the page title
    await expect(page.getByRole('heading', { name: /live monitoring/i })).toBeVisible();
  });

  test('shows control bar with stop and clear buttons', async ({ page }) => {
    await page.goto('/live');

    // Should have control bar
    await expect(page.getByTestId('control-bar')).toBeVisible();

    // Should have stop button
    await expect(page.getByTestId('stop-button')).toBeVisible();

    // Should have clear button
    await expect(page.getByTestId('clear-button')).toBeVisible();
  });

  test('stop button is disabled when no loop running', async ({ page }) => {
    await page.goto('/live');

    // Stop button should be disabled when no loop is running
    await expect(page.getByTestId('stop-button')).toBeDisabled();
  });

  test('shows loop status component', async ({ page }) => {
    await page.goto('/live');

    // Should have loop status display
    await expect(page.getByTestId('live-dashboard')).toBeVisible();
  });

  test('displays output pane', async ({ page }) => {
    await page.goto('/live');

    // Should show output section (CardTitle renders as div with specific slot)
    await expect(page.locator('[data-slot="card-title"]', { hasText: 'Output' })).toBeVisible();
  });
});
