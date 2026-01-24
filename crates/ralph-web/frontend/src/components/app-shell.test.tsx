/**
 * Tests for AppShell component
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { AppShell } from './app-shell';

function renderWithRouter() {
  return render(
    <MemoryRouter>
      <AppShell />
    </MemoryRouter>
  );
}

describe('AppShell', () => {
  beforeEach(() => {
    // Reset document class
    document.documentElement.className = '';
  });

  it('renders navigation links', () => {
    renderWithRouter();

    expect(screen.getByRole('link', { name: /sessions/i })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /live/i })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /start loop/i })).toBeInTheDocument();
  });

  it('renders app branding', () => {
    renderWithRouter();

    expect(screen.getByText('Ralph')).toBeInTheDocument();
  });

  it('renders theme toggle button', () => {
    renderWithRouter();

    expect(screen.getByRole('button', { name: /toggle theme/i })).toBeInTheDocument();
  });
});
