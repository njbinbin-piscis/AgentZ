import { invoke } from "@tauri-apps/api/core";

export type NoticeKind = "meaningful_change" | "completed" | "failed" | "action_required";
export interface TaskNotice {
  event_key: string;
  task_id: string;
  kind: NoticeKind;
  summary: string;
  created_ms: number;
  read_ms: number | null;
  /** Sidebar title of the chat this task belongs to. */
  session_title?: string;
  /** Latest non-empty assistant message, collapsed to one line. */
  last_reply?: string;
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
const NOTICE_KEYS = ["event_key", "task_id", "kind", "summary", "created_ms", "read_ms"] as const;
const OPTIONAL_TEXT_KEYS = ["session_title", "last_reply"] as const;

function text(value: unknown, max: number): asserts value is string {
  if (typeof value !== "string" || value.length > max || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/u.test(value))
    throw new Error("Invalid inbox text");
}

function optionalText(record: Record<string, unknown>, key: (typeof OPTIONAL_TEXT_KEYS)[number], max: number): string | undefined {
  if (!(key in record) || record[key] == null) return undefined;
  text(record[key], max);
  const value = (record[key] as string).trim();
  return value ? value : undefined;
}

function notice(value: unknown): TaskNotice {
  const n = object(value);
  const keys = Object.keys(n);
  if (keys.some((key) => !NOTICE_KEYS.includes(key as (typeof NOTICE_KEYS)[number]) && !OPTIONAL_TEXT_KEYS.includes(key as (typeof OPTIONAL_TEXT_KEYS)[number])))
    throw new Error("Invalid inbox notice");
  if (NOTICE_KEYS.some((key) => !(key in n))) throw new Error("Invalid inbox notice");
  id(n.event_key, 512); id(n.task_id, 256); id(n.summary, 240);
  if (!["meaningful_change", "completed", "failed", "action_required"].includes(n.kind as string)) throw new Error("Invalid inbox kind");
  time(n.created_ms); if (n.read_ms !== null) time(n.read_ms);
  return {
    event_key: n.event_key as string,
    task_id: n.task_id as string,
    kind: n.kind as NoticeKind,
    summary: n.summary as string,
    created_ms: n.created_ms as number,
    read_ms: n.read_ms as number | null,
    session_title: optionalText(n, "session_title", 240),
    last_reply: optionalText(n, "last_reply", 4500),
  };
}

export async function listNotifications(scopeHandle: string, limit = 50): Promise<InboxPage> {
  id(scopeHandle, 256);
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) throw new Error("Invalid inbox limit");
  const page = object(await invoke("notification_inbox_list", { scopeHandle, limit }));
  if (Object.keys(page).length !== 2 || !Array.isArray(page.notices) || page.notices.length > limit) throw new Error("Invalid inbox page");
  time(page.unread_count);
  return { notices: page.notices.map(notice), unread_count: page.unread_count as number };
}

/** Marks the notice and, when `taskId` is given, every other notice of that task. */
export async function markNotificationRead(scopeHandle: string, eventKey: string, taskId?: string): Promise<boolean> {
  id(scopeHandle, 256); id(eventKey, 512);
  if (taskId !== undefined) id(taskId, 256);
  const result: unknown = await invoke("notification_inbox_mark_read", { scopeHandle, eventKey, taskId: taskId ?? null });
  if (typeof result !== "boolean") throw new Error("Invalid inbox mark-read result");
  return result;
}

export async function clearNotifications(scopeHandle: string): Promise<number> {
  id(scopeHandle, 256);
  const result: unknown = await invoke("notification_inbox_clear", { scopeHandle });
  if (!Number.isSafeInteger(result) || (result as number) < 0) throw new Error("Invalid inbox clear result");
  return result as number;
}
