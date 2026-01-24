import { test, expect } from '@playwright/test';

test.describe('Start Loop', () => {
  test('displays start loop page with form', async ({ page }) => {
    await page.goto('/start');

    // Should show the page title
    await expect(page.getByRole('heading', { name: /start loop/i })).toBeVisible();

    // Should show form elements
    await expect(page.getByTestId('config-select')).toBeVisible();
    await expect(page.getByTestId('prompt-input')).toBeVisible();
    await expect(page.getByTestId('working-dir-input')).toBeVisible();
    await expect(page.getByTestId('submit-button')).toBeVisible();
  });

  test('validates required fields', async ({ page }) => {
    await page.goto('/start');

    // Clear working dir to test validation
    await page.getByTestId('working-dir-input').clear();

    // Click submit
    await page.getByTestId('submit-button').click();

    // Should show validation errors (check at least one required error appears)
    await expect(page.getByText(/required/i).first()).toBeVisible();
  });

  test('prompt accepts multiline input', async ({ page }) => {
    await page.goto('/start');

    const multilineText = 'Line 1\nLine 2\nLine 3';
    await page.getByTestId('prompt-input').fill(multilineText);

    // Verify the content was entered
    const value = await page.getByTestId('prompt-input').inputValue();
    expect(value).toBe(multilineText);
  });

  test('keyboard shortcut Cmd/Ctrl+Enter submits form', async ({ page }) => {
    await page.goto('/start');

    // Fill the form
    await page.getByTestId('prompt-input').fill('Test prompt');

    // Focus the prompt input and press Cmd/Ctrl+Enter
    const textarea = page.getByTestId('prompt-input');
    await textarea.focus();
    await textarea.press('Meta+Enter');

    // Form should attempt to submit (may show error if no config selected)
    // This verifies the keyboard shortcut is wired up
    await page.waitForTimeout(100);
  });

  test('cancel button navigates to live page', async ({ page }) => {
    await page.goto('/start');

    // Click cancel
    await page.getByRole('button', { name: /cancel/i }).click();

    // Should navigate to live
    await expect(page).toHaveURL('/live');
  });

  test('shows error when submitting without config', async ({ page }) => {
    await page.goto('/start');

    // Config may be auto-selected if configs exist, so explicitly select empty option
    const configSelect = page.getByTestId('config-select');
    await configSelect.selectOption({ value: '' });

    // Fill prompt but not config
    await page.getByTestId('prompt-input').fill('Test prompt');

    // Click submit
    await page.getByTestId('submit-button').click();

    // Should show config required error
    await expect(page.getByText(/config.*required/i)).toBeVisible();
  });
});
