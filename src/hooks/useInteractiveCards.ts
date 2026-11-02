import { useCallback, useEffect, useMemo, useState } from "react";
import type { AgentEvent } from "../services/tauri/chat";
import { applyUiPatch } from "../components/chat/interactiveUi/patch";
import type { UiDefinition, UiPatch } from "../components/chat/interactiveUi/protocol";

export interface InteractiveCardState {
  requestId: string;
  uiDefinition: UiDefinition;
  submitted?: boolean;
  listenOpen?: boolean;
  wizardStepHint?: number;
  /** While the backend tool is waiting for an answer: when it gives up (epoch ms). */
  waitDeadlineMs?: number;
}

/** The chat_ui / chat_ui_listen tools stop waiting for the user after 5 minutes. */
export const INTERACTIVE_WAIT_MS = 300_000;

export function useInteractiveCards() {
  const [cards, setCards] = useState<Record<string, InteractiveCardState>>({});

  const handleAgentEvent = useCallback((evt: AgentEvent) => {
    switch (evt.type) {
      case "interactive_ui":
        setCards((prev) => ({
          ...prev,
          [evt.request_id]: {
            requestId: evt.request_id,
            uiDefinition: evt.ui_definition as UiDefinition,
            listenOpen: false,
            waitDeadlineMs: Date.now() + INTERACTIVE_WAIT_MS,
          },
        }));
        break;
      case "interactive_ui_patch": {
        const patch = evt.patch as UiPatch;
        setCards((prev) => {
          const card = prev[evt.request_id];
          if (!card) return prev;
          return {
            ...prev,
            [evt.request_id]: {
              ...card,
              uiDefinition: applyUiPatch(card.uiDefinition, patch),
              listenOpen: patch.reopen_submit === true ? true : card.listenOpen,
              wizardStepHint: patch.wizard_step ?? card.wizardStepHint,
            },
          };
        });
        break;
      }
      case "interactive_ui_listen":
        setCards((prev) => {
          const card = prev[evt.request_id];
          if (!card) return prev;
          return {
            ...prev,
            [evt.request_id]: {
              ...card,
              listenOpen: true,
              waitDeadlineMs: Date.now() + INTERACTIVE_WAIT_MS,
            },
          };
        });
        break;
      case "done":
      case "cancelled":
        setCards((prev) => {
          const kept: Record<string, InteractiveCardState> = {};
          for (const [id, card] of Object.entries(prev)) {
            const kind = (card.uiDefinition as UiDefinition).kind;
            if (kind === "plan_mode_build" && !card.submitted) {
              kept[id] = card;
            }
          }
          return kept;
        });
        break;
      default:
        break;
    }
  }, []);

  // The tool call that owned a card gave up after its timeout and the agent
  // carried on with defaults, so the form is no longer actionable: retire it
  // instead of leaving it pinned below the conversation.
  useEffect(() => {
    const deadlines = Object.values(cards)
      .filter((c) => !c.submitted && c.waitDeadlineMs != null && !c.uiDefinition.kind?.startsWith("plan_mode"))
      .map((c) => c.waitDeadlineMs as number);
    if (deadlines.length === 0) return;
    const delay = Math.max(0, Math.min(...deadlines) - Date.now());
    const timer = window.setTimeout(() => {
      const now = Date.now();
      setCards((prev) => {
        let changed = false;
        const next: Record<string, InteractiveCardState> = {};
        for (const [id, card] of Object.entries(prev)) {
          if (!card.submitted && card.waitDeadlineMs != null && card.waitDeadlineMs <= now
            && !card.uiDefinition.kind?.startsWith("plan_mode")) {
            changed = true;
            next[id] = { ...card, submitted: true, listenOpen: false, waitDeadlineMs: undefined };
          } else {
            next[id] = card;
          }
        }
        return changed ? next : prev;
      });
    }, delay);
    return () => window.clearTimeout(timer);
  }, [cards]);

  const pendingCards = useMemo(
    () => Object.values(cards).filter((c) => !c.submitted),
    [cards],
  );

  const markSubmitted = useCallback((requestId: string) => {
    setCards((prev) => {
      const card = prev[requestId];
      if (!card) return prev;
      return {
        ...prev,
        [requestId]: { ...card, submitted: true, listenOpen: false, waitDeadlineMs: undefined },
      };
    });
  }, []);

  const markActionSent = useCallback((requestId: string) => {
    setCards((prev) => {
      const card = prev[requestId];
      if (!card) return prev;
      return {
        ...prev,
        [requestId]: { ...card, listenOpen: false, waitDeadlineMs: undefined },
      };
    });
  }, []);

  const clearCards = useCallback(() => setCards({}), []);

  return {
    pendingCards,
    handleAgentEvent,
    markSubmitted,
    markActionSent,
    clearCards,
  };
}
