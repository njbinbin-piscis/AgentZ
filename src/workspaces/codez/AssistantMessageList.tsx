import { memo, useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { JournalFileDiff } from "../../services/tauri/chat";
import TaskCard from "../../components/TaskCard";
import FileDiffCard from "../../components/FileDiffCard";
import InteractiveCard from "../../components/chat/InteractiveCard";
import PermissionCard, { type PermissionRequestCard } from "../../components/chat/PermissionCard";
import ToolTrace, { interleaveTools, type ToolTraceItem } from "../../components/ToolTrace";
import type { InteractiveCardState } from "../../hooks/useInteractiveCards";
import { InteractiveCardsContext } from "../../components/chat/InteractiveCardsContext";
import { useRestoreChatUiCards } from "../../hooks/useInteractiveCards";
import type { UiDefinition } from "../../components/chat/interactiveUi/protocol";

const NO_CARDS: Record<string, InteractiveCardState> = {};
const NOOP_RESTORE = () => {};
import Markdown from "./Markdown";

export interface AssistantChatMessage {
  id?: string;
  role: "user" | "assistant";
  text: string;
  turnId?: string;
  /** Persisted tool calls (history / interrupted turns). */
  tools?: ToolTraceItem[];
}

interface AssistantMessageListProps {
  messages: AssistantChatMessage[];
  /** Tool calls for the in-flight turn — rendered inline under the last reply. */
  toolSteps: ToolTraceItem[];
  turnDiffsByTurnId: Record<string, JournalFileDiff[]>;
  busy: boolean;
  queuedView: string[];
  onRemoveQueued?: (index: number) => void;
  pendingCards: InteractiveCardState[];
  /** Every card of this session (answered ones included), keyed by request id. */
  allCards?: Record<string, InteractiveCardState>;
  onRestoreCard?: (requestId: string, uiDefinition: UiDefinition) => void;
  scrollRef: RefObject<HTMLDivElement>;
  /** Older history exists in the DB beyond what is loaded in `messages`. */
  hasMoreOlder?: boolean;
  /** Fetch + prepend the next-older page; resolves with the number of messages added. */
  onLoadOlder?: () => Promise<number>;
  onSelectPath?: (path: string) => void;
  onForkCheckpoint: (messageId: string, fullHistory: boolean) => void | Promise<void>;
  onRestoreCheckpoint: (messageId: string) => void | Promise<void>;
  onUndoTurn?: (turnId: string) => void | Promise<void>;
  onCardSubmitted: (requestId: string, values?: Record<string, unknown>) => void;
  onCardActionSent: (requestId: string) => void;
  onPlanModeChange?: (mode: "plan" | "agent") => void;
  onPlanBuild?: (planPath: string) => void;
  permissionRequest: PermissionRequestCard | null;
  onPermissionResolved: () => void;
}

/** How many of the newest messages stay mounted by default. */
const INITIAL_VISIBLE = 50;
/** Extra older messages mounted per scroll-up / "load older" click. */
const LOAD_MORE = 10;
/** Distance from the top (px) that trips the lazy-load of older messages. */
const LOAD_AT_TOP_PX = 40;

function ForkMenu({ onFork }: { onFork: (fullHistory: boolean) => void }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const pick = (fullHistory: boolean) => {
    setOpen(false);
    onFork(fullHistory);
  };

  return (
    <div className="agentz-fork-menu" ref={rootRef}>
      <button
        type="button"
        className="agentz-checkpoint-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        title={t("chat.checkpointFork")}
      >
        {t("chat.checkpointFork")} ▾
      </button>
      {open && (
        <div className="agentz-fork-popover" role="menu">
          <button type="button" role="menuitem" onClick={() => pick(false)}>
            {t("chat.forkContextOnly")}
            <span>{t("chat.forkContextOnlyHint")}</span>
          </button>
          <button type="button" role="menuitem" onClick={() => pick(true)}>
            {t("chat.forkFullHistory")}
            <span>{t("chat.forkFullHistoryHint")}</span>
          </button>
        </div>
      )}
    </div>
  );
}

/** Isolated from composer input state so keystrokes do not re-render markdown. */
function AssistantMessageList({
  messages,
  toolSteps,
  turnDiffsByTurnId,
  busy,
  queuedView,
  onRemoveQueued,
  pendingCards,
  allCards,
  onRestoreCard,
  scrollRef,
  hasMoreOlder = false,
  onLoadOlder,
  onSelectPath,
  onForkCheckpoint,
  onRestoreCheckpoint,
  onUndoTurn,
  onCardSubmitted,
  onCardActionSent,
  onPlanModeChange,
  onPlanBuild,
  permissionRequest,
  onPermissionResolved,
}: AssistantMessageListProps) {
  const { t } = useTranslation();
  // The mounted window is anchored to the BOTTOM: it always contains the newest
  // message, and only ever grows backwards when the user scrolls up. Bounding it
  // to a count (rather than an absolute start index) makes it impossible for a
  // newly appended message to fall outside the window, and keeps the DOM bounded
  // in multi-hour sessions.
  const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE);
  const loadLockRef = useRef(false);

  const startIndex = Math.max(0, messages.length - visibleCount);
  const hasHiddenOlder = startIndex > 0 || hasMoreOlder;

  const visibleMessages = useMemo(() => messages.slice(startIndex), [messages, startIndex]);

  const lastUserMessageIndex = useMemo(() => {
    for (let i = visibleMessages.length - 1; i >= 0; i--) {
      if (visibleMessages[i].role === "user" && visibleMessages[i].text.trim()) return i;
    }
    return -1;
  }, [visibleMessages]);

  // Mount LOAD_MORE older messages while keeping the reading position stable:
  // whatever was under the viewport before must stay exactly there after the
  // rows above it appear.
  const loadOlder = useCallback(async () => {
    const el = scrollRef.current;
    if (!el || loadLockRef.current) return;
    const needFetch = startIndex <= 0;
    if (needFetch && (!hasMoreOlder || !onLoadOlder)) return;
    loadLockRef.current = true;
    const prevScrollHeight = el.scrollHeight;
    const prevScrollTop = el.scrollTop;
    let grow = LOAD_MORE;
    if (needFetch) {
      // Everything already loaded is mounted: pull the next-older page from the DB.
      try {
        grow = await onLoadOlder!();
      } catch {
        grow = 0;
      }
      if (grow <= 0) {
        loadLockRef.current = false;
        return;
      }
    }
    setVisibleCount((c) => c + grow);
    requestAnimationFrame(() => {
      const node = scrollRef.current;
      if (node) {
        node.scrollTop = prevScrollTop + (node.scrollHeight - prevScrollHeight);
      }
      loadLockRef.current = false;
    });
  }, [scrollRef, startIndex, hasMoreOlder, onLoadOlder]);

  const onScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el || loadLockRef.current || !hasHiddenOlder) return;
    if (el.scrollTop > LOAD_AT_TOP_PX) return;
    void loadOlder();
  }, [scrollRef, hasHiddenOlder, loadOlder]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [onScroll, scrollRef]);

  const chatUiItems = useMemo(
    () => [...messages.flatMap((m) => m.tools ?? []), ...toolSteps].filter((tool) => tool.name === "chat_ui"),
    [messages, toolSteps],
  );
  const settledIds = useRestoreChatUiCards(chatUiItems, allCards ?? NO_CARDS, onRestoreCard ?? NOOP_RESTORE);
  const cardsContext = useMemo(
    () => ({
      cards: allCards ?? {},
      settledIds: onRestoreCard ? settledIds : undefined,
      onSubmitted: onCardSubmitted,
      onActionSent: onCardActionSent,
      onPlanModeChange,
      onPlanBuild,
    }),
    [allCards, settledIds, onRestoreCard, onCardSubmitted, onCardActionSent, onPlanModeChange, onPlanBuild],
  );
  // Forms that already sit in the message flow (as chat_ui tool steps) are not
  // repeated at the bottom.
  const inlineCardIds = useMemo(() => {
    const ids = new Set<string>();
    for (const m of messages) for (const tool of m.tools ?? []) if (tool.name === "chat_ui") ids.add(tool.id);
    for (const tool of toolSteps) if (tool.name === "chat_ui") ids.add(tool.id);
    return ids;
  }, [messages, toolSteps]);
  const bottomCards = useMemo(
    () => pendingCards.filter((c) => !inlineCardIds.has(c.requestId)),
    [pendingCards, inlineCardIds],
  );

  return (
    <InteractiveCardsContext.Provider value={cardsContext}>
    <div className="agentz-assistant-messages" ref={scrollRef}>
      {hasHiddenOlder && (
        <button type="button" className="agentz-load-more-msgs" onClick={() => void loadOlder()}>
          {t("chat.loadOlderMessages", { count: startIndex > 0 ? Math.min(LOAD_MORE, startIndex) : LOAD_MORE })}
        </button>
      )}
      {messages.length === 0 && <div className="agentz-assistant-empty">{t("chat.empty")}</div>}
      {visibleMessages.map((m, i) => {
        const absoluteIndex = startIndex + i;
        if (m.role === "user") {
          return (
            <div key={m.id ?? `msg-${absoluteIndex}`} className="agentz-turn-user">
              <TaskCard text={m.text} sticky={i === lastUserMessageIndex} />
            </div>
          );
        }

        const isLastMessage = absoluteIndex === messages.length - 1;
        const isStreamingLast = busy && isLastMessage;
        const showCheckpoint = m.id && m.text.trim() && !isStreamingLast;
        // Edit diffs are process info: only the newest turn of this run shows them.
        // Earlier turns and restored history drop them (no turnId once reloaded).
        const showEdits = isLastMessage && (busy || !!m.turnId);
        const diffs = showEdits && m.turnId ? turnDiffsByTurnId[m.turnId] : undefined;
        // Live steps drive the in-flight turn; otherwise use what was persisted.
        const msgTools =
          isLastMessage && (busy || !m.tools?.length) && toolSteps.length > 0
            ? toolSteps
            : (m.tools ?? []);

        return (
          <div key={m.id ?? `msg-${absoluteIndex}`} className="agentz-msg assistant">
            {msgTools.length > 0 ? (
              interleaveTools(m.text, msgTools).map((seg, si, all) => (
                <div key={`seg-${si}`} className="agentz-msg-seg">
                  {seg.text ? <Markdown content={seg.text} /> : null}
                  {seg.tools.length > 0 && <ToolTrace items={seg.tools} hideEdits={!showEdits} />}
                  {isStreamingLast && si === all.length - 1 && !seg.text && seg.tools.length === 0 && (
                    <div className="agentz-msg-text agentz-thinking">{t("chat.thinking")}</div>
                  )}
                </div>
              ))
            ) : m.text ? (
              <Markdown content={m.text} />
            ) : isStreamingLast ? (
              <div className="agentz-msg-text agentz-thinking">{t("chat.thinking")}</div>
            ) : null}
            {diffs && diffs.length > 0 && (
              <div className="agentz-turn-diffs">
                {diffs.map((d) => (
                  <FileDiffCard key={d.id} diff={d} onOpen={onSelectPath} />
                ))}
                {m.turnId && onUndoTurn && !isStreamingLast && (
                  <div className="agentz-turn-undo-bar">
                    <button
                      type="button"
                      data-testid={`undo-turn-${m.turnId}`}
                      className="agentz-checkpoint-btn"
                      onClick={() => void onUndoTurn(m.turnId!)}
                    >
                      {t("agent.rollbackTurn")}
                    </button>
                  </div>
                )}
              </div>
            )}
            {showCheckpoint && (
              <div className="agentz-checkpoint">
                <span className="agentz-checkpoint-label">{t("chat.checkpoint")}</span>
                <ForkMenu onFork={(fullHistory) => void onForkCheckpoint(m.id!, fullHistory)} />
                <button
                  type="button"
                  className="agentz-checkpoint-btn muted"
                  onClick={() => void onRestoreCheckpoint(m.id!)}
                  title={t("chat.checkpointRestore")}
                >
                  {t("chat.checkpointRestore")}
                </button>
              </div>
            )}
          </div>
        );
      })}
      {queuedView.map((q, i) => (
        <div key={`q-${i}`} className="agentz-turn-user queued">
          <TaskCard text={q} />
          {onRemoveQueued && (
            <button
              type="button"
              className="agentz-queue-remove"
              title={t("chat.removeFromQueue")}
              onClick={() => onRemoveQueued(i)}
            >
              ✕
            </button>
          )}
        </div>
      ))}
      {bottomCards.map((card) => (
        <div key={card.requestId} className="agentz-msg assistant">
          <InteractiveCard
            requestId={card.requestId}
            uiDefinition={card.uiDefinition}
            listenOpen={card.listenOpen}
            wizardStepHint={card.wizardStepHint}
            onSubmitted={(values) => onCardSubmitted(card.requestId, values)}
            onActionSent={() => onCardActionSent(card.requestId)}
            onPlanModeChange={onPlanModeChange}
            onPlanBuild={onPlanBuild}
          />
        </div>
      ))}
      {permissionRequest && (
        <PermissionCard request={permissionRequest} onResolved={onPermissionResolved} />
      )}
    </div>
    </InteractiveCardsContext.Provider>
  );
}

export default memo(AssistantMessageList);
