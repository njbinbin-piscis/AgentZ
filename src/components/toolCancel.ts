/**
 * Results the agent loop writes for tool calls that a user Stop cut short.
 * They are reported as errors, but nothing went wrong: showing them as failed
 * calls made a deliberate stop look like a crash.
 *
 * The texts mirror the kernel constants `TOOL_RESULT_NOT_STARTED`,
 * `TOOL_RESULT_CANCELLED_BEFORE_RUN` and `TOOL_RESULT_INTERRUPTED`
 * (piscis-kernel `agent/loop_.rs`); keep the two in step.
 */
const NOT_STARTED = "Tool execution was cancelled or interrupted before completion.";
const CANCELLED_BEFORE_RUN = "已取消";
const INTERRUPTED = "已被用户取消";

/** Live `tool_end` events carry a trace prefix that the persisted result does not. */
const TRACE_PREFIX = /^\[trace_id:[^\]]*\]\s*/;

/**
 * - `skipped`: the model asked for the call, the stop arrived before it was
 *   dispatched. It was never shown while the turn ran.
 * - `interrupted`: the call had started (or was about to) and was cut short.
 */
export type ToolCancelKind = "skipped" | "interrupted";

export function toolCancelKind(result: string | null | undefined, isError: boolean): ToolCancelKind | null {
  if (!isError || !result) return null;
  const body = result.replace(TRACE_PREFIX, "").trim();
  if (body === NOT_STARTED) return "skipped";
  if (body === CANCELLED_BEFORE_RUN || body === INTERRUPTED) return "interrupted";
  return null;
}

export type SettledToolStatus = "done" | "error" | "cancelled";

/** Status of a finished call, from its result text and error flag. */
export function settledToolStatus(
  result: string | null | undefined,
  isError: boolean,
): { status: SettledToolStatus; skipped?: true } {
  const kind = toolCancelKind(result, isError);
  if (kind === "skipped") return { status: "cancelled", skipped: true };
  if (kind === "interrupted") return { status: "cancelled" };
  return { status: isError ? "error" : "done" };
}

/** Calls that actually ran: the number shown on a collapsed batch. */
export function countRunTools(items: readonly { skipped?: boolean }[]): number {
  let n = 0;
  for (const item of items) if (!item.skipped) n += 1;
  return n;
}
