/**
 * Tests for ThemeToggle component
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ThemeToggle } from './theme-toggle';
import { useAppStore } from '@/lib/store';

beforeEach(() => {
  // Reset store state (not setState which triggers persist middleware)
  useAppStore.getState().setTheme('system');
});

describe('ThemeToggle', () => {
  it('toggles dark mode', () => {
    render(<ThemeToggle />);

    const button = screen.getByRole('button', { name: /toggle theme/i });

    // Initial state is system
    expect(useAppStore.getState().theme).toBe('system');

    // Click to switch to light
    fireEvent.click(button);
    expect(useAppStore.getState().theme).toBe('light');

    // Click to switch to dark
    fireEvent.click(button);
    expect(useAppStore.getState().theme).toBe('dark');

    // Click to switch back to system
    fireEvent.click(button);
    expect(useAppStore.getState().theme).toBe('system');
  });
});
