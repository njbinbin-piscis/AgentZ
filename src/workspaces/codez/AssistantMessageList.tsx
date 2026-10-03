import { memo, useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { JournalFileDiff } from "../../services/tauri/chat";
import TaskCard from "../../components/TaskCard";
import FileDiffCard from "../../components/FileDiffCard";
import InteractiveCard from "../../components/chat/InteractiveCard";
import PermissionCard, { type PermissionRequestCard } from "../../components/chat/PermissionCard";
import ToolTrace, { interleaveTools, type ToolTraceItem } from "../../components/ToolTrace";
import type { InteractiveCardState } from "../../hooks/useInteractiveCards";
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
  scrollRef: RefObject<HTMLDivElement>;
  /** Older history exists in the DB beyond what is loaded in `messages`. */
  hasMoreOlder?: boolean;
  /** Fetch + prepend the next-older page; resolves with the number of messages added. */
  onLoadOlder?: () => Promise<number>;
  onSelectPath?: (path: string) => void;
  onForkCheckpoint: (messageId: string) => void | Promise<void>;
  onRestoreCheckpoint: (messageId: string) => void | Promise<void>;
  onCardSubmitted: (requestId: string) => void;
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

/** Isolated from composer input state so keystrokes do not re-render markdown. */
function AssistantMessageList({
  messages,
  toolSteps,
  turnDiffsByTurnId,
  busy,
  queuedView,
  onRemoveQueued,
  pendingCards,
  scrollRef,
  hasMoreOlder = false,
  onLoadOlder,
  onSelectPath,
  onForkCheckpoint,
  onRestoreCheckpoint,
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

  return (
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
        const diffs = m.turnId ? turnDiffsByTurnId[m.turnId] : undefined;
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
                  {seg.tools.length > 0 && <ToolTrace items={seg.tools} />}
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
              </div>
            )}
            {showCheckpoint && (
              <div className="agentz-checkpoint">
                <span className="agentz-checkpoint-label">{t("chat.checkpoint")}</span>
                <button
                  type="button"
                  className="agentz-checkpoint-btn"
                  onClick={() => void onForkCheckpoint(m.id!)}
                  title={t("chat.checkpointFork")}
                >
                  {t("chat.checkpointFork")}
                </button>
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
      {pendingCards.map((card) => (
        <div key={card.requestId} className="agentz-msg assistant">
          <InteractiveCard
            requestId={card.requestId}
            uiDefinition={card.uiDefinition}
            listenOpen={card.listenOpen}
            wizardStepHint={card.wizardStepHint}
            onSubmitted={() => onCardSubmitted(card.requestId)}
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
  );
}

export default memo(AssistantMessageList);
