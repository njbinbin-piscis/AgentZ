import type { AgentEvent } from "../../services/tauri/chat";

/** What an avatar is visibly doing at its desk. */
export type Activity = "idle" | "thinking" | "typing" | "reading" | "running" | "waiting" | "error";

const TYPING_TOOLS = /(^|_)(file_)?(edit|write|create|patch|apply|replace|notebook)/i;
const READING_TOOLS = /(read|grep|glob|search|list|find|lsp|wiki|fetch|browse|view|code_map|symbols)/i;
const RUNNING_TOOLS = /(shell|code_run|run|exec|terminal|test|build|background|git)/i;

export function activityForTool(name: string): Activity {
  if (TYPING_TOOLS.test(name)) return "typing";
  if (READING_TOOLS.test(name)) return "reading";
  if (RUNNING_TOOLS.test(name)) return "running";
  return "thinking";
}

/** Next activity after an event, or null when the event does not change it. */
export function activityFromEvent(evt: AgentEvent, current: Activity): Activity | null {
  switch (evt.type) {
    case "text_segment_start":
    case "text_delta":
      return current === "thinking" ? null : "thinking";
    case "tool_start":
      return activityForTool(evt.name);
    case "tool_end":
      return evt.is_error ? "error" : "thinking";
    case "permission_request":
    case "interactive_ui":
      return "waiting";
    case "done":
    case "cancelled":
      return "idle";
    case "error":
      return "error";
    default:
      return null;
  }
}

/** Member turns stream under `koi_task_{koiId}_{todoId[0..8]}`. */
export const koiTaskSessionId = (koiId: string, todoId: string) => `koi_task_${koiId}_${todoId.slice(0, 8)}`;

/** Resolve which member a koi task session belongs to, preferring the longest id match. */
export function koiIdFromSession(sessionId: string, koiIds: string[]): string | null {
  if (!sessionId.startsWith("koi_task_")) return null;
  const rest = sessionId.slice("koi_task_".length);
  let best: string | null = null;
  for (const id of koiIds) {
    if (rest.startsWith(`${id}_`) && (!best || id.length > best.length)) best = id;
  }
  return best;
}

/** Deterministic desk grid: `cols` desks per row, coordinator desk excluded. */
export function deskGrid(count: number, maxCols = 4): { cols: number; rows: number } {
  if (count <= 0) return { cols: 1, rows: 0 };
  const cols = Math.min(maxCols, Math.max(2, Math.ceil(Math.sqrt(count))));
  return { cols, rows: Math.ceil(count / cols) };
}
