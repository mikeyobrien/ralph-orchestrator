/**
 * Root App component with routing
 */

import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ThemeProvider } from '@/components/theme-provider';
import { ErrorBoundary } from '@/components/error-boundary';
import { AppShell } from '@/components/app-shell';
import { IndexRoute } from '@/routes/index';
import { SessionsRoute } from '@/routes/sessions';
import { SessionDetailRoute } from '@/routes/session-detail';
import { LiveRoute } from '@/routes/live';
import { StartRoute } from '@/routes/start';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60, // 1 minute
      retry: 1,
    },
  },
});

function App() {
  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <BrowserRouter>
            <Routes>
              <Route element={<AppShell />}>
                <Route index element={<IndexRoute />} />
                <Route path="sessions" element={<SessionsRoute />} />
                <Route path="sessions/:id" element={<SessionDetailRoute />} />
                <Route path="live" element={<LiveRoute />} />
                <Route path="live/:sessionId" element={<LiveRoute />} />
                <Route path="start" element={<StartRoute />} />
              </Route>
            </Routes>
          </BrowserRouter>
        </ThemeProvider>
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

export default App;
