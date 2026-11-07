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

/**
 * Run `send`; while the backend says this session's gate is held (a background
 * continuation or heartbeat is running), wait and try again, like CodeZ does.
 * Stops waiting when `shouldContinue` turns false (user pressed Stop / left).
 * Resolves with the last result, or throws the last error.
 */
export async function sendWhenGateFree<T>(
  send: () => Promise<T>,
  shouldContinue: () => boolean = () => true,
  { attempts = 150, delayMs = 2000 }: { attempts?: number; delayMs?: number } = {},
): Promise<T> {
  let last: { ok: true; value: T } | { ok: false; error: unknown };
  const run = async () => {
    try {
      last = { ok: true, value: await send() };
    } catch (error) {
      last = { ok: false, error };
    }
  };
  await run();
  for (let i = 0; i < attempts && !last!.ok && isTaskGateBusy(last!.error); i++) {
    await new Promise((resolve) => setTimeout(resolve, delayMs));
    if (!shouldContinue()) break;
    await run();
  }
  if (last!.ok) return last!.value;
  throw last!.error;
}
