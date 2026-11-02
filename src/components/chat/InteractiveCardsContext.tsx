import { createContext } from "react";
import type { InteractiveCardState } from "../../hooks/useInteractiveCards";

/**
 * Lets a `chat_ui` tool step in the message flow find its live form (if the
 * backend is still waiting on it) and report submissions back to the session.
 */
export interface InteractiveCardsContextValue {
  cards: Record<string, InteractiveCardState>;
  /** chat_ui calls confirmed not to be waiting any more (no live form, no result). */
  settledIds?: Set<string>;
  onSubmitted: (requestId: string, values?: Record<string, unknown>) => void;
  onActionSent: (requestId: string) => void;
  onPlanModeChange?: (mode: "plan" | "agent") => void;
  onPlanBuild?: (planPath: string) => void;
}

export const InteractiveCardsContext = createContext<InteractiveCardsContextValue | null>(null);

const RESPONSE_MARKER = "USER_INTERACTIVE_RESPONSE_JSON:\n";

/** Recover the submitted values from a persisted `chat_ui` / `chat_ui_listen` tool result. */
export function parseInteractiveResponse(result: string | undefined): Record<string, unknown> | null {
  if (!result) return null;
  const start = result.indexOf(RESPONSE_MARKER);
  if (start < 0) return null;
  const body = result.slice(start + RESPONSE_MARKER.length);
  const end = body.indexOf("\n\n");
  try {
    const parsed: unknown = JSON.parse(end < 0 ? body : body.slice(0, end));
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

/** `ui_definition` from a tool step's input (object, or JSON text in restored history). */
export function parseChatUiDefinition(input: unknown): Record<string, unknown> | null {
  let value = input;
  if (typeof value === "string") {
    try {
      value = JSON.parse(value);
    } catch {
      return null;
    }
  }
  const def = (value as { ui_definition?: unknown } | null)?.ui_definition;
  return def && typeof def === "object" && Array.isArray((def as { blocks?: unknown }).blocks)
    ? (def as Record<string, unknown>)
    : null;
}
