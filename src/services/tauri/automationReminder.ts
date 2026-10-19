import { invoke } from "@tauri-apps/api/core";

export interface ReminderStatus {
  interval_minutes: number;
  next_due_ms: number;
}

function validId(value: string): void {
  // eslint-disable-next-line no-control-regex
  if (!value.trim() || value.length > 256 || /[\u0000-\u001f\u007f]/u.test(value)) throw new Error("Invalid reminder identity");
}

function status(value: unknown): ReminderStatus {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Invalid reminder status");
  const item = value as Record<string, unknown>;
  if (Object.keys(item).length !== 2 || !Number.isSafeInteger(item.interval_minutes) ||
      (item.interval_minutes as number) < 5 || (item.interval_minutes as number) > 10080 ||
      !Number.isSafeInteger(item.next_due_ms) || (item.next_due_ms as number) < 0)
    throw new Error("Invalid reminder status");
  return item as unknown as ReminderStatus;
}

export async function reminderStatus(scopeHandle: string, taskId: string): Promise<ReminderStatus | null> {
  validId(scopeHandle); validId(taskId);
  const value: unknown = await invoke("automation_reminder_status", { scopeHandle, taskId });
  return value === null ? null : status(value);
}

export async function setReminder(scopeHandle: string, taskId: string, intervalMinutes: number): Promise<ReminderStatus> {
  validId(scopeHandle); validId(taskId);
  if (!Number.isSafeInteger(intervalMinutes) || intervalMinutes < 5 || intervalMinutes > 10080)
    throw new Error("Invalid reminder interval");
  return status(await invoke("automation_reminder_set", { scopeHandle, taskId, intervalMinutes }));
}

export async function disableReminder(scopeHandle: string, taskId: string): Promise<void> {
  validId(scopeHandle); validId(taskId);
  const value: unknown = await invoke("automation_reminder_disable", { scopeHandle, taskId });
  if (value !== null && value !== undefined) throw new Error("Invalid reminder disable response");
}

export async function pollReminders(scopeHandle: string): Promise<number> {
  validId(scopeHandle);
  const value: unknown = await invoke("automation_reminder_poll", { scopeHandle });
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > 25)
    throw new Error("Invalid reminder poll response");
  return value as number;
}
