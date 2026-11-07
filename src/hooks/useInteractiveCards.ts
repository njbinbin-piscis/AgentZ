import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { pendingInteractiveIds } from "../services/tauri/interactive";
import type { ToolTraceItem } from "../components/ToolTrace";
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
  /** What the user submitted, kept so the frozen card still shows it. */
  submittedValues?: Record<string, unknown>;
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
      case "tool_end": {
        // The tool that was waiting on the user failed (timeout, cancel, error):
        // nobody is listening any more, so the form is dead. chat_ui's
        // tool_use_id is the card's request id; chat_ui_listen belongs to
        // whichever card was listening.
        const timedOut = evt.is_error || /timed out/i.test(evt.result ?? "");
        if (!timedOut) break;
        const isListen = evt.name === "chat_ui_listen";
        if (evt.name !== "chat_ui" && !isListen) break;
        setCards((prev) => {
          let changed = false;
          const next: Record<string, InteractiveCardState> = {};
          for (const [id, card] of Object.entries(prev)) {
            const match = isListen ? card.listenOpen === true : id === evt.id;
            if (match && !card.submitted && !card.uiDefinition.kind?.startsWith("plan_mode")) {
              changed = true;
              next[id] = { ...card, submitted: true, listenOpen: false, waitDeadlineMs: undefined };
            } else {
              next[id] = card;
            }
          }
          return changed ? next : prev;
        });
        break;
      }
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

  const markSubmitted = useCallback((requestId: string, values?: Record<string, unknown>) => {
    setCards((prev) => {
      const card = prev[requestId];
      if (!card) return prev;
      return {
        ...prev,
        [requestId]: {
          ...card,
          submitted: true,
          listenOpen: false,
          waitDeadlineMs: undefined,
          submittedValues: values ?? card.submittedValues,
        },
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

  /**
   * The plan-ready Build card outlives its turn on purpose, so the user can
   * accept it after the plan finishes. A later message means they did not
   * accept it: retire the card instead of leaving it pinned under the chat.
   */
  const dismissPlanBuilds = useCallback(() => {
    setCards((prev) => {
      let changed = false;
      const next: Record<string, InteractiveCardState> = {};
      for (const [id, card] of Object.entries(prev)) {
        if (!card.submitted && card.uiDefinition.kind === "plan_mode_build") {
          changed = true;
          next[id] = { ...card, submitted: true, listenOpen: false, waitDeadlineMs: undefined };
        } else {
          next[id] = card;
        }
      }
      return changed ? next : prev;
    });
  }, []);

  /** Re-create a card whose backend tool call is still waiting (view was reloaded). */
  const restoreCard = useCallback((requestId: string, uiDefinition: UiDefinition) => {
    setCards((prev) =>
      prev[requestId]
        ? prev
        : { ...prev, [requestId]: { requestId, uiDefinition, listenOpen: false } },
    );
  }, []);

  return {
    cards,
    pendingCards,
    handleAgentEvent,
    markSubmitted,
    markActionSent,
    clearCards,
    dismissPlanBuilds,
    restoreCard,
  };
}

/** `ui_definition` of a chat_ui tool step (object, or JSON text from history). */
function definitionOf(item: ToolTraceItem): UiDefinition | null {
  let input: unknown = item.input;
  if (typeof input === "string") {
    try {
      input = JSON.parse(input);
    } catch {
      return null;
    }
  }
  const def = (input as { ui_definition?: unknown } | null)?.ui_definition as UiDefinition | undefined;
  return def && Array.isArray(def.blocks) ? def : null;
}

/**
 * A reloaded view (session switched away and back) shows a chat_ui call as
 * result-less. If the backend is in fact still waiting on it, bring its form
 * back to life so the user does not wait out the timeout. Returns the ids
 * confirmed NOT waiting, so the UI can show them as unanswered instead of
 * guessing while the check is in flight.
 */
export function useRestoreChatUiCards(
  items: ToolTraceItem[],
  cards: Record<string, InteractiveCardState>,
  restoreCard: (requestId: string, uiDefinition: UiDefinition) => void,
): Set<string> {
  const [settled, setSettled] = useState<Set<string>>(() => new Set());
  const pendingRef = useRef<Set<string>>(new Set());
  const checkedRef = useRef<Set<string>>(new Set());

  const unresolved = useMemo(
    () => items.filter((i) => i.name === "chat_ui" && i.result == null && !cards[i.id]),
    [items, cards],
  );

  useEffect(() => {
    // Known to be waiting but the card was cleared by a view reset: bring it back.
    for (const item of unresolved) {
      if (!pendingRef.current.has(item.id)) continue;
      const def = definitionOf(item);
      if (def) restoreCard(item.id, def);
    }
    const fresh = unresolved.filter((i) => !checkedRef.current.has(i.id));
    if (fresh.length === 0) return;
    for (const i of fresh) checkedRef.current.add(i.id);
    let cancelled = false;
    void pendingInteractiveIds()
      .catch(() => [] as string[])
      .then((ids) => {
        if (cancelled) return;
        const waiting = new Set(ids);
        const done: string[] = [];
        for (const item of fresh) {
          const def = definitionOf(item);
          if (waiting.has(item.id) && def) {
            pendingRef.current.add(item.id);
            restoreCard(item.id, def);
          } else {
            done.push(item.id);
          }
        }
        if (done.length > 0) setSettled((prev) => new Set([...prev, ...done]));
      });
    return () => {
      cancelled = true;
      for (const i of fresh) checkedRef.current.delete(i.id);
    };
  }, [unresolved, restoreCard]);

  return settled;
}
