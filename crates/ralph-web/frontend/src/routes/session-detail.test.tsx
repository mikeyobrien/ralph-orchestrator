/**
 * Tests for SessionDetailRoute component
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { userEvent } from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SessionDetailRoute } from './session-detail';
import * as apiModule from '@/lib/api';

// Mock the api module
vi.mock('@/lib/api', () => ({
  api: {
    getSession: vi.fn(),
    getIterationContent: vi.fn(),
    searchSession: vi.fn(),
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

function renderWithProviders(
  ui: React.ReactElement,
  { initialRoute = '/sessions/test-session' }: { initialRoute?: string } = {}
) {
  const queryClient = createTestQueryClient();
  return {
    user: userEvent.setup(),
    ...render(
      <QueryClientProvider client={queryClient}>
        <MemoryRouter initialEntries={[initialRoute]}>
          <Routes>
            <Route path="/sessions/:id" element={ui} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  };
}

const mockSession: apiModule.Session = {
  id: '2026-01-21T10-33-47',
  started_at: '2026-01-21T10:33:47',
  iterations: [
    {
      number: 1,
      hat: { id: 'ralph', display: 'Ralph' },
      events: [
        { topic: 'build.start', payload: null, timestamp: '2026-01-21T10:33:47Z' },
      ],
    },
    {
      number: 2,
      hat: { id: 'builder', display: '⚙️ Builder' },
      events: [
        { topic: 'implementation.ready', payload: 'tests pass', timestamp: '2026-01-21T10:34:00Z' },
      ],
    },
    {
      number: 3,
      hat: { id: 'validator', display: '✅ Validator' },
      events: [
        { topic: 'validation.passed', payload: null, timestamp: '2026-01-21T10:35:00Z' },
      ],
    },
  ],
  status: 'completed',
};

const mockIterationContent: apiModule.IterationContent = {
  lines: [
    { text: 'Starting implementation...', line_type: 'text', timestamp: '2026-01-21T10:33:47Z' },
    { text: 'Writing test file...', line_type: 'text', timestamp: '2026-01-21T10:33:48Z' },
    { text: 'Running tests...', line_type: 'tool_call', timestamp: '2026-01-21T10:33:49Z' },
    { text: 'Tests passed!', line_type: 'tool_result', timestamp: '2026-01-21T10:33:50Z' },
  ],
  events: [
    { topic: 'build.start', payload: null, timestamp: '2026-01-21T10:33:47Z' },
  ],
};

describe('SessionDetailRoute', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  it('displays loading state', async () => {
    vi.mocked(apiModule.api.getSession).mockImplementation(
      () => new Promise(() => {})
    );

    renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/2026-01-21T10-33-47',
    });

    expect(screen.getByTestId('session-detail-loading')).toBeInTheDocument();
  });

  it('displays session metadata', async () => {
    vi.mocked(apiModule.api.getSession).mockResolvedValue(mockSession);
    vi.mocked(apiModule.api.getIterationContent).mockResolvedValue(mockIterationContent);

    renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/2026-01-21T10-33-47',
    });

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    expect(screen.getByText('completed')).toBeInTheDocument();
    expect(screen.getByText(/3 iterations/i)).toBeInTheDocument();
  });

  it('navigates between iterations', async () => {
    vi.mocked(apiModule.api.getSession).mockResolvedValue(mockSession);
    vi.mocked(apiModule.api.getIterationContent).mockResolvedValue(mockIterationContent);

    const { user } = renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/2026-01-21T10-33-47',
    });

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    // Should show iteration 1 by default
    expect(screen.getByText(/iteration 1/i)).toBeInTheDocument();

    // Click next to go to iteration 2
    const nextButton = screen.getByRole('button', { name: /next/i });
    await user.click(nextButton);

    expect(screen.getByText(/iteration 2/i)).toBeInTheDocument();

    // Click prev to go back to iteration 1
    const prevButton = screen.getByRole('button', { name: /prev/i });
    await user.click(prevButton);

    expect(screen.getByText(/iteration 1/i)).toBeInTheDocument();
  });

  it('searches and highlights matches', async () => {
    vi.mocked(apiModule.api.getSession).mockResolvedValue(mockSession);
    vi.mocked(apiModule.api.getIterationContent).mockResolvedValue(mockIterationContent);

    const { user } = renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/2026-01-21T10-33-47',
    });

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    // Type in search box
    const searchInput = screen.getByPlaceholderText(/search/i);
    await user.type(searchInput, 'test');

    // Should show search results count
    await waitFor(() => {
      expect(screen.getByTestId('search-results')).toBeInTheDocument();
    });
  });

  it('handles error state', async () => {
    vi.mocked(apiModule.api.getSession).mockRejectedValue(
      new Error('Session not found')
    );

    renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/nonexistent',
    });

    await waitFor(() => {
      expect(screen.getByTestId('session-detail-error')).toBeInTheDocument();
    });

    expect(screen.getByText(/failed to load session/i)).toBeInTheDocument();
  });

  it('displays iteration content', async () => {
    vi.mocked(apiModule.api.getSession).mockResolvedValue(mockSession);
    vi.mocked(apiModule.api.getIterationContent).mockResolvedValue(mockIterationContent);

    renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/2026-01-21T10-33-47',
    });

    await waitFor(() => {
      expect(screen.getByText('Starting implementation...')).toBeInTheDocument();
    });

    expect(screen.getByText('Writing test file...')).toBeInTheDocument();
    expect(screen.getByText('Running tests...')).toBeInTheDocument();
    expect(screen.getByText('Tests passed!')).toBeInTheDocument();
  });

  it('disables prev button on first iteration', async () => {
    vi.mocked(apiModule.api.getSession).mockResolvedValue(mockSession);
    vi.mocked(apiModule.api.getIterationContent).mockResolvedValue(mockIterationContent);

    renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/2026-01-21T10-33-47',
    });

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    const prevButton = screen.getByRole('button', { name: /prev/i });
    expect(prevButton).toBeDisabled();
  });

  it('disables next button on last iteration', async () => {
    vi.mocked(apiModule.api.getSession).mockResolvedValue(mockSession);
    vi.mocked(apiModule.api.getIterationContent).mockResolvedValue(mockIterationContent);

    const { user } = renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/2026-01-21T10-33-47',
    });

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    // Navigate to last iteration
    const nextButton = screen.getByRole('button', { name: /next/i });
    await user.click(nextButton); // iteration 2
    await user.click(nextButton); // iteration 3

    expect(nextButton).toBeDisabled();
  });

  it('displays hat information for current iteration', async () => {
    vi.mocked(apiModule.api.getSession).mockResolvedValue(mockSession);
    vi.mocked(apiModule.api.getIterationContent).mockResolvedValue(mockIterationContent);

    const { user } = renderWithProviders(<SessionDetailRoute />, {
      initialRoute: '/sessions/2026-01-21T10-33-47',
    });

    await waitFor(() => {
      expect(screen.getByText('2026-01-21T10:33:47')).toBeInTheDocument();
    });

    // First iteration has Ralph hat
    expect(screen.getByText('Ralph')).toBeInTheDocument();

    // Navigate to iteration 2 with Builder hat
    const nextButton = screen.getByRole('button', { name: /next/i });
    await user.click(nextButton);

    await waitFor(() => {
      expect(screen.getByText('⚙️ Builder')).toBeInTheDocument();
    });
  });
});
