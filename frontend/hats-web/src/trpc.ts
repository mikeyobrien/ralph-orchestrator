/**
 * TRPC Client Configuration
 *
 * Sets up the TRPC client with React Query integration.
 * Imports the AppRouter type from the server for end-to-end type safety.
 */

import { createTRPCReact } from "@trpc/react-query";
import { httpBatchLink } from "@trpc/client";
import type { AppRouter } from "@hats-web/server/src/api/trpc";

/**
 * TRPC React hooks - provides useQuery, useMutation etc.
 * The generic type ensures all procedures are fully typed.
 */
export const trpc = createTRPCReact<AppRouter>();

/**
 * Create the TRPC client with HTTP batch link.
 * In dev mode, Vite proxies /trpc to localhost:3000.
 * In production, this would be the actual API URL.
 */
export function createTRPCClient() {
  return trpc.createClient({
    links: [
      httpBatchLink({
        url: "/trpc",
      }),
    ],
  });
}
