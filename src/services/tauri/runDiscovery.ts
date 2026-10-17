import { invoke } from "@tauri-apps/api/core";

export type RunExecution =
  | "queued"
  | "running"
  | "cancelling"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "interrupted";
export type RunDelivery = "not_ready" | "awaiting_review" | "accepted" | "rejected";
export interface CurrentTaskRun {
  readonly run_id: string;
  readonly task_id: string;
  readonly environment_id: string;
  readonly attempt_id: string;
  readonly execution: RunExecution;
  readonly delivery: RunDelivery;
  readonly updated_ms: number;
}

const executions = new Set<string>([
  "queued",
  "running",
  "cancelling",
  "succeeded",
  "failed",
  "cancelled",
  "interrupted",
]);
const deliveries = new Set<string>(["not_ready", "awaiting_review", "accepted", "rejected"]);

function identity(value: unknown): asserts value is string {
  if (
    typeof value !== "string" ||
    !value ||
    value.trim() !== value ||
    new TextEncoder().encode(value).length > 256 ||
    /\p{Cc}/u.test(value)
  ) {
    throw new Error("Invalid current Run identity");
  }
}

/** Passive discovery of the host's current Run/Attempt; never starts or retries it. */
export async function readCurrentTaskRun(
  scopeHandle: string,
  sessionId: string,
): Promise<CurrentTaskRun | null> {
  identity(scopeHandle);
  identity(sessionId);
  const value = await invoke<unknown>("chat_current_run", { scopeHandle, sessionId });
  if (value === null) return null;
  if (!value || typeof value !== "object") throw new Error("Invalid current Run response");
  const run = value as Record<string, unknown>;
  for (const key of ["run_id", "task_id", "environment_id", "attempt_id"]) identity(run[key]);
  if (run.task_id !== sessionId) throw new Error("Current Run belongs to another task");
  if (
    typeof run.execution !== "string" ||
    !executions.has(run.execution) ||
    typeof run.delivery !== "string" ||
    !deliveries.has(run.delivery)
  ) {
    throw new Error("Unsupported current Run state");
  }
  if ((run.execution === "succeeded") === (run.delivery === "not_ready"))
    throw new Error("Inconsistent current Run delivery");
  if (
    typeof run.updated_ms !== "number" ||
    !Number.isSafeInteger(run.updated_ms) ||
    run.updated_ms < 0
  )
    throw new Error("Invalid current Run timestamp");
  return Object.freeze({
    run_id: run.run_id as string,
    task_id: run.task_id as string,
    environment_id: run.environment_id as string,
    attempt_id: run.attempt_id as string,
    execution: run.execution as RunExecution,
    delivery: run.delivery as RunDelivery,
    updated_ms: run.updated_ms,
  });
}
