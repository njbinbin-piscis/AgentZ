import { memo, useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { JournalFileDiff } from "../../services/tauri/chat";
import TaskCard from "../../components/TaskCard";
import FileDiffCard from "../../components/FileDiffCard";
import InteractiveCard from "../../components/chat/InteractiveCard";
import PermissionCard, { type PermissionRequestCard } from "../../components/chat/PermissionCard";
import type { InteractiveCardState } from "../../hooks/useInteractiveCards";
import Markdown from "./Markdown";

export interface AssistantChatMessage {
  id?: string;
  role: "user" | "assistant";
  text: string;
  turnId?: string;
}

interface AssistantMessageListProps {
  messages: AssistantChatMessage[];
  turnDiffsByTurnId: Record<string, JournalFileDiff[]>;
  busy: boolean;
  queuedView: string[];
  onRemoveQueued?: (index: number) => void;
  pendingCards: InteractiveCardState[];
  scrollRef: RefObject<HTMLDivElement>;
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

/** Isolated from composer input state so keystrokes do not re-render markdown. */
function AssistantMessageList({
  messages,
  turnDiffsByTurnId,
  busy,
  queuedView,
  onRemoveQueued,
  pendingCards,
  scrollRef,
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
  // Keep the DOM bounded even for multi-hour agent sessions. The persisted
  // session remains intact; older turns are mounted only when the user scrolls
  // to the history boundary.
  const INITIAL_VISIBLE = 50;
  const LOAD_MORE = 10;
  const [visibleFrom, setVisibleFrom] = useState(0);
  const loadLockRef = useRef(false);

  useEffect(() => {
    setVisibleFrom((prev) => {
      const next = Math.max(0, messages.length - INITIAL_VISIBLE);
      return messages.length > prev + LOAD_MORE ? next : Math.min(prev, next);
    });
  }, [messages.length]);

  const onScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el || loadLockRef.current || visibleFrom <= 0) return;
    if (el.scrollTop > 40) return;
    loadLockRef.current = true;
    const prevTop = el.scrollHeight;
    setVisibleFrom((v) => Math.max(0, v - LOAD_MORE));
    requestAnimationFrame(() => {
      const node = scrollRef.current;
      if (node) node.scrollTop = node.scrollHeight - prevTop;
      loadLockRef.current = false;
    });
  }, [scrollRef, visibleFrom]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [onScroll, scrollRef]);

  const visibleMessages = useMemo(
    () => messages.slice(visibleFrom),
    [messages, visibleFrom],
  );

  const lastUserMessageIndex = useMemo(() => {
    for (let i = visibleMessages.length - 1; i >= 0; i--) {
      if (visibleMessages[i].role === "user" && visibleMessages[i].text.trim()) return i;
    }
    return -1;
  }, [visibleMessages]);

  return (
    <div className="agentz-assistant-messages" ref={scrollRef}>
      {visibleFrom > 0 && (
        <button
          type="button"
          className="agentz-load-more-msgs"
          onClick={() => setVisibleFrom((v) => Math.max(0, v - LOAD_MORE))}
        >
          {t("chat.loadOlderMessages", { count: visibleFrom })}
        </button>
      )}
      {messages.length === 0 && <div className="agentz-assistant-empty">{t("chat.empty")}</div>}
      {visibleMessages.map((m, i) => {
        if (m.role === "user") {
          return (
            <div key={m.id ?? `msg-${visibleFrom + i}`} className="agentz-turn-user agentz-msg-virtual">
              <TaskCard text={m.text} sticky={i === lastUserMessageIndex} />
            </div>
          );
        }

        const isStreamingLast = busy && visibleFrom + i === messages.length - 1;
        const showCheckpoint = m.id && m.text.trim() && !isStreamingLast;
        const diffs = m.turnId ? turnDiffsByTurnId[m.turnId] : undefined;

          return (
            <div key={m.id ?? `msg-${visibleFrom + i}`} className="agentz-msg assistant agentz-msg-virtual">
              {m.text ? (
                isStreamingLast ? (
                  <pre className="agentz-msg-text agentz-streaming">{m.text}</pre>
                ) : (
                  <Markdown content={m.text} />
                )
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
