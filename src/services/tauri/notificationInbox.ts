import { invoke } from "@tauri-apps/api/core";

export type NoticeKind = "meaningful_change" | "completed" | "failed" | "action_required";
export interface TaskNotice {
  event_key: string;
  task_id: string;
  kind: NoticeKind;
  summary: string;
  created_ms: number;
  read_ms: number | null;
}
export interface InboxPage { notices: TaskNotice[]; unread_count: number }

function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid inbox response");
  return value as Record<string, unknown>;
}
function id(value: unknown, max: number): asserts value is string {
  // eslint-disable-next-line no-control-regex
  if (typeof value !== "string" || !value.trim() || value.length > max || /[\u0000-\u001f\u007f]/u.test(value))
    throw new Error("Invalid inbox identity");
}
function time(value: unknown): asserts value is number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) throw new Error("Invalid inbox timestamp");
}
function notice(value: unknown): TaskNotice {
  const n = object(value);
  if (Object.keys(n).length !== 6) throw new Error("Invalid inbox notice");
  id(n.event_key, 512); id(n.task_id, 256); id(n.summary, 240);
  if (!["meaningful_change", "completed", "failed", "action_required"].includes(n.kind as string)) throw new Error("Invalid inbox kind");
  time(n.created_ms); if (n.read_ms !== null) time(n.read_ms);
  return n as unknown as TaskNotice;
}

export async function listNotifications(scopeHandle: string, limit = 50): Promise<InboxPage> {
  id(scopeHandle, 256);
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) throw new Error("Invalid inbox limit");
  const page = object(await invoke("notification_inbox_list", { scopeHandle, limit }));
  if (Object.keys(page).length !== 2 || !Array.isArray(page.notices) || page.notices.length > limit) throw new Error("Invalid inbox page");
  time(page.unread_count);
  return { notices: page.notices.map(notice), unread_count: page.unread_count as number };
}

export async function markNotificationRead(scopeHandle: string, eventKey: string): Promise<boolean> {
  id(scopeHandle, 256); id(eventKey, 512);
  const result: unknown = await invoke("notification_inbox_mark_read", { scopeHandle, eventKey });
  if (typeof result !== "boolean") throw new Error("Invalid inbox mark-read result");
  return result;
}
