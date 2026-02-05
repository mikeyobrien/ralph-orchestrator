/**
 * Plan Page - Main page component for planning workflow
 *
 * Routes between PlanLanding (start new plan) and PlanSession (active plan)
 */

import { useState } from "react";
import { PlanLanding, PlanSession } from "@/components/plan";

export function PlanPage() {
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);

  if (!activeSessionId) {
    return <PlanLanding onStart={(id) => setActiveSessionId(id)} />;
  }

  return (
    <PlanSession
      sessionId={activeSessionId}
      onBack={() => setActiveSessionId(null)}
    />
  );
}
