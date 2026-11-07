export function isChatSendResult(
  value: unknown,
): value is { ok: boolean; session_id: string; response_text: string; turn_id?: string | null } {
  if (!value || typeof value !== "object") return false;
  const rec = value as Record<string, unknown>;
  return typeof rec.session_id === "string" && typeof rec.response_text === "string";
}

/** Backend refuses a second turn while this session's gate is still held. */
export function isTaskGateBusy(error: unknown): boolean {
  return String(error).includes("task queued/running");
}

/** Seconds from `phase=running,4731s`, or null when the holder note has no age. */
export function taskGateHoldSeconds(error: unknown): number | null {
  const match = /,(\d+)s\)/.exec(String(error));
  if (!match) return null;
  const seconds = Number(match[1]);
  return Number.isFinite(seconds) ? seconds : null;
}
