/**
 * Tests for SessionsRoute component
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import { userEvent } from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SessionsRoute } from './sessions';
import * as api from '@/lib/api';

// Mock the api module
vi.mock('@/lib/api', () => ({
  api: {
    listSessions: vi.fn(),
  },
}));

function createTestQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });
}

function renderWithProviders(ui: React.ReactElement) {
  const queryClient = createTestQueryClient();
  return {
    user: userEvent.setup(),
    ...render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>{ui}</MemoryRouter>
      </QueryClientProvider>
    ),
  };
}

const mockSessions: api.SessionSummary[] = [
  {
    id: '2026-01-21T10-33-47',
    started_at: '2026-01-21T10:33:47',
    iteration_count: 5,
    status: 'completed',
  },
  {
    id: '2026-01-20T08-15-22',
    started_at: '2026-01-20T08:15:22',
    iteration_count: 3,
    status: 'failed',
  },
  {
    id: '2026-01-19T14-22-00',
    started_at: '2026-01-19T14:22:00',
    iteration_count: 1,
    status: 'running',
  },
];

describe('SessionsRoute', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  it('displays loading state', async () => {
    // Never resolve the promise to keep loading state
    vi.mocked(api.api.listSessions).mockImplementation(
      () => new Promise(() => {})
    );

    renderWithProviders(<SessionsRoute />);

    expect(screen.getByTestId('sessions-loading')).toBeInTheDocument();
  });

  it('displays sessions from API', async () => {
    vi.mocked(api.api.listSessions).mockResolvedValue(mockSessions);

    renderWithProviders(<SessionsRoute />);

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    expect(screen.getByText('5 iterations')).toBeInTheDocument();
    expect(screen.getByText('3 iterations')).toBeInTheDocument();
    expect(screen.getByText('1 iteration')).toBeInTheDocument();
  });

  it('navigates to detail on click', async () => {
    vi.mocked(api.api.listSessions).mockResolvedValue(mockSessions);

    renderWithProviders(<SessionsRoute />);

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    // Each session card should be a link to session detail
    const sessionLinks = screen.getAllByRole('link');
    const firstSessionLink = sessionLinks.find((link) =>
      link.getAttribute('href')?.includes('/sessions/2026-01-21T10-33-47')
    );
    expect(firstSessionLink).toBeInTheDocument();
  });

  it('shows empty state when no sessions', async () => {
    vi.mocked(api.api.listSessions).mockResolvedValue([]);

    renderWithProviders(<SessionsRoute />);

    await waitFor(() => {
      expect(screen.getByTestId('sessions-empty')).toBeInTheDocument();
    });

    expect(screen.getByText(/no sessions found/i)).toBeInTheDocument();
  });

  it('shows error state when API fails', async () => {
    vi.mocked(api.api.listSessions).mockRejectedValue(
      new Error('Network error')
    );

    renderWithProviders(<SessionsRoute />);

    await waitFor(() => {
      expect(screen.getByTestId('sessions-error')).toBeInTheDocument();
    });

    expect(screen.getByText(/failed to load sessions/i)).toBeInTheDocument();
  });

  it('displays status badges correctly', async () => {
    vi.mocked(api.api.listSessions).mockResolvedValue(mockSessions);

    renderWithProviders(<SessionsRoute />);

    await waitFor(() => {
      expect(screen.getByText('completed')).toBeInTheDocument();
    });

    expect(screen.getByText('failed')).toBeInTheDocument();
    expect(screen.getByText('running')).toBeInTheDocument();
  });

  it('filters sessions by status', async () => {
    vi.mocked(api.api.listSessions).mockResolvedValue(mockSessions);

    const { user } = renderWithProviders(<SessionsRoute />);

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    // Find and click the "completed" filter button
    const filterButtons = screen.getByTestId('status-filter');
    const completedButton = within(filterButtons).getByRole('button', {
      name: /completed/i,
    });
    await user.click(completedButton);

    // Only completed sessions should be visible
    expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    expect(screen.queryByText('2026-01-20T08:15:22')).not.toBeInTheDocument();
    expect(screen.queryByText('2026-01-19T14:22:00')).not.toBeInTheDocument();
  });

  it('sorts sessions by date (newest first)', async () => {
    vi.mocked(api.api.listSessions).mockResolvedValue(mockSessions);

    renderWithProviders(<SessionsRoute />);

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    const sessionCards = screen.getAllByTestId('session-card');
    expect(sessionCards).toHaveLength(3);

    // First card should be the newest (2026-01-21)
    expect(
      within(sessionCards[0]).getByText('2026-01-21T10:33:47')
    ).toBeInTheDocument();
  });
});
