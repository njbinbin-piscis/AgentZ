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

/** `Parent: <todo id>` line the coordinator writes into a subtask description. */
export function parentRef(description: string): string | null {
  const m = /^\s*parent:\s*`?([A-Za-z0-9-]{6,})/im.exec(description);
  return m ? m[1] : null;
}

/** The parent todo of `todo` within the same pool, if it declares one. */
export function parentOf<T extends { id: string; description: string }>(todo: T, todos: T[]): T | null {
  const ref = parentRef(todo.description);
  if (!ref) return null;
  return todos.find((p) => p.id !== todo.id && p.id.startsWith(ref)) ?? null;
}

/** Whether a pool message addresses `name` with `@name` or `@!name`. */
export function mentionsMember(content: string, name: string): boolean {
  if (!name) return false;
  const esc = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`@!?${esc}(?![\\p{L}\\p{N}_])`, "iu").test(content);
}

/** Messages after `lastReadId` that mention `name`, sent by someone else. */
export function unreadFor(
  feed: { id: number; sender_id: string; content: string }[],
  koiId: string,
  name: string,
  lastReadId: number,
): number {
  return feed.filter((m) => m.id > lastReadId && m.sender_id !== koiId && mentionsMember(m.content, name)).length;
}
