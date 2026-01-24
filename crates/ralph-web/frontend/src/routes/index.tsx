/**
 * Index route - redirects to sessions
 */

import { Navigate } from 'react-router-dom';

export function IndexRoute() {
  return <Navigate to="/sessions" replace />;
}
