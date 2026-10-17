import { invoke } from "@tauri-apps/api/core";

export interface DurablePlanBinding {
  project_id: string;
  task_id: string;
  environment_id: string;
  binding_version: string;
}
export interface DurablePlan {
  id: string;
  binding: DurablePlanBinding;
  revision: number;
  objective: string;
  items: { id: string; description: string; state: "pending" | "in_progress" | "done" | "cancelled" }[];
  status: "active" | "completed" | "cancelled" | "replaced";
  created_ms: number;
  updated_ms: number;
}
export interface DurablePlanQuery {
  binding: DurablePlanBinding;
  current: DurablePlan | null;
}
export type ExplicitPlanIntent = { kind: "replace"; objective: string } | { kind: "cancel" };

/** Query persisted state; rendering it never implies replacing a task goal. */
export function queryTaskPlan(scopeHandle: string, sessionId: string): Promise<DurablePlanQuery> {
  return invoke("chat_plan_query", { scopeHandle, sessionId });
}

/** One explicit action against the displayed revision; never retry a rejected CAS. */
export function applyTaskPlanIntent(
  scopeHandle: string,
  sessionId: string,
  expected: Pick<DurablePlan, "id" | "revision">,
  intent: ExplicitPlanIntent,
): Promise<DurablePlan> {
  if (!expected.id.trim() || !Number.isSafeInteger(expected.revision) || expected.revision < 1) {
    return Promise.reject(new Error("Expected plan identity and revision required"));
  }
  if (intent.kind === "replace" && !intent.objective.trim()) {
    return Promise.reject(new Error("Explicit objective required"));
  }
  return invoke("chat_plan_apply", { scopeHandle, sessionId, expected, intent });
}
