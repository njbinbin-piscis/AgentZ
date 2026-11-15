import type { TFunction } from "i18next";

/** A kernel `notice` event as it arrives on the chat channel. */
export interface AgentNotice {
  message: string;
  code?: string | null;
  details?: unknown;
}

function detail(details: unknown, key: string): string | null {
  if (!details || typeof details !== "object" || Array.isArray(details)) return null;
  const value = (details as Record<string, unknown>)[key];
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  return null;
}

/**
 * Banner text for a kernel notice.
 *
 * The kernel writes its messages in Chinese. Notices with a known code and
 * complete details are shown in the app's language; everything else falls
 * back to the kernel's own message, so no notice is ever dropped.
 */
export function agentNoticeText(t: TFunction, notice: AgentNotice): string {
  const fallback = typeof notice.message === "string" ? notice.message.trim() : "";
  switch (notice.code) {
    case "context_static_overhead": {
      const percent = detail(notice.details, "percent");
      if (percent) return t("chat.noticeStaticOverhead", { percent });
      break;
    }
    case "read_loop_paused":
      return t("chat.noticeReadLoopPaused");
    case "model_fallback": {
      const from = detail(notice.details, "from");
      const to = detail(notice.details, "to");
      if (from && to) return t("chat.noticeModelFallback", { from, to });
      break;
    }
    default:
      break;
  }
  return fallback;
}

const MIN_TOAST_MS = 4500;
const MAX_TOAST_MS = 12000;

/**
 * How long a toast stays up: long enough to read it. A CJK character counts
 * for more reading time than a Latin letter.
 */
export function toastDurationMs(text: string): number {
  let ms = 0;
  for (const char of text) ms += /[\u2e80-\u9fff\uf900-\ufaff\uff00-\uffef]/.test(char) ? 150 : 45;
  return Math.min(MAX_TOAST_MS, Math.max(MIN_TOAST_MS, ms));
}
