import { Navigate } from 'react-router-dom';

/** Legacy route: history merged into tasks. */
export function History() {
  return <Navigate to="/processing" replace />;
}
