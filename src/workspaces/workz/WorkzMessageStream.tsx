import { useMemo, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import { journalUndoTurn } from "../../services/tauri/chat";
import { confirmDialog } from "../../services/tauri/confirm";
import PermissionCard, { type PermissionRequestCard } from "../../components/chat/PermissionCard";
import InteractiveCard from "../../components/chat/InteractiveCard";
import { InteractiveCardsContext } from "../../components/chat/InteractiveCardsContext";
import { useRestoreChatUiCards } from "../../hooks/useInteractiveCards";
import type { UiDefinition } from "../../components/chat/interactiveUi/protocol";

const NO_CARDS: Record<string, InteractiveCardState> = {};
const NOOP_RESTORE = () => {};
import type { InteractiveCardState } from "../../hooks/useInteractiveCards";
import ToolTrace, { interleaveTools } from "../../components/ToolTrace";
import Markdown from "../codez/Markdown";
import { ForkMenu } from "../codez/AssistantMessageList";
import PoolActivityFeed from "./PoolActivityFeed";
import WorkzEmptyState, { type WorkzStarterScenario } from "./WorkzEmptyState";
import type { AgentStep } from "./agentArtifacts";
import WorkzQueuedList from "./WorkzQueuedList";

export interface WorkzMessageStreamProps {
  projectDir: string | null;
  isFree: boolean;
  freeTaskWorkspace: string | null;
  isSwarmTeam: boolean;
  swarmMainTab: "main" | "chatroom" | "coordination";
  onChangeSwarmTab: (tab: "main" | "chatroom" | "coordination") => void;
  activePoolId: string | null;
  scrollRef: RefObject<HTMLDivElement>;
  steps: AgentStep[];
  busy: boolean;
  selectedId: string | null;
  sessionId: string | null;
  pendingCards: InteractiveCardState[];
  allCards?: Record<string, InteractiveCardState>;
  onRestoreCard?: (requestId: string, uiDefinition: UiDefinition) => void;
  permissionRequest: PermissionRequestCard | null;
  onClearPermission: () => void;
  onMarkCardSubmitted: (requestId: string, values?: Record<string, unknown>) => void;
  onMarkCardActionSent: (requestId: string) => void;
  onSelectScenario: (scenario: WorkzStarterScenario) => void;
  onRefreshGitChanges: () => void;
  onShowToast: (msg: string) => void;
  onSetError: (err: string) => void;
  queued?: string[];
  onInterruptQueued?: (index: number) => void;
  onSteerQueued?: (index: number) => void;
  onEditQueued?: (index: number) => void;
  onRemoveQueued?: (index: number) => void;
  onForkCheckpoint?: (messageId: string, fullHistory: boolean) => void | Promise<void>;
  onRestoreCheckpoint?: (messageId: string) => void | Promise<void>;
}

export default function WorkzMessageStream({
  projectDir,
  isFree,
  freeTaskWorkspace,
  isSwarmTeam,
  swarmMainTab,
  onChangeSwarmTab,
  activePoolId,
  scrollRef,
  steps,
  busy,
  selectedId,
  sessionId,
  pendingCards,
  allCards,
  onRestoreCard,
  permissionRequest,
  onClearPermission,
  onMarkCardSubmitted,
  onMarkCardActionSent,
  onSelectScenario,
  onRefreshGitChanges,
  onShowToast,
  onSetError,
  queued = [],
  onInterruptQueued,
  onSteerQueued,
  onEditQueued,
  onRemoveQueued,
  onForkCheckpoint,
  onRestoreCheckpoint,
}: WorkzMessageStreamProps) {
  const { t } = useTranslation();
  const chatUiItems = useMemo(
    () => steps.flatMap((m) => (m.role === "assistant" ? m.tools : [])).filter((tool) => tool.name === "chat_ui"),
    [steps],
  );
  const settledIds = useRestoreChatUiCards(chatUiItems, allCards ?? NO_CARDS, onRestoreCard ?? NOOP_RESTORE);
  const cardsContext = useMemo(
    () => ({
      cards: allCards ?? {},
      settledIds: onRestoreCard ? settledIds : undefined,
      onSubmitted: onMarkCardSubmitted,
      onActionSent: onMarkCardActionSent,
    }),
    [allCards, settledIds, onRestoreCard, onMarkCardSubmitted, onMarkCardActionSent],
  );
  const bottomCards = useMemo(() => {
    const inline = new Set<string>();
    for (const m of steps) if (m.role === "assistant") for (const tool of m.tools) if (tool.name === "chat_ui") inline.add(tool.id);
    return pendingCards.filter((c) => !inline.has(c.requestId));
  }, [steps, pendingCards]);

  return (
    <InteractiveCardsContext.Provider value={cardsContext}>
    <div className="agentz-workz-steps-wrap">
      {isSwarmTeam && projectDir && (
        <div className="agentz-workz-swarm-tabs">
          <button
            type="button"
            className={`agentz-workz-swarm-tab${swarmMainTab === "main" ? " active" : ""}`}
            onClick={() => onChangeSwarmTab("main")}
          >
            {t("agent.swarmTabMain")}
          </button>
          <button
            type="button"
            className={`agentz-workz-swarm-tab${swarmMainTab === "chatroom" ? " active" : ""}`}
            onClick={() => onChangeSwarmTab("chatroom")}
          >
            {t("agent.swarmTabChatroom")}
          </button>
          <button
            type="button"
            className={`agentz-workz-swarm-tab${swarmMainTab === "coordination" ? " active" : ""}`}
            onClick={() => onChangeSwarmTab("coordination")}
          >
            {t("agent.swarmTabCoordination")}
          </button>
        </div>
      )}
      {isSwarmTeam && projectDir && swarmMainTab !== "main" ? (
        activePoolId ? (
          <PoolActivityFeed
            projectDir={projectDir}
            poolId={activePoolId}
            filter={swarmMainTab === "chatroom" ? "chat" : "events"}
          />
        ) : (
          <div className="agentz-workz-feed-loading">{t("common.loading")}</div>
        )
      ) : (
        <div className="agentz-workz-steps" ref={scrollRef}>
          {steps.length === 0 && (
            <WorkzEmptyState
              title={t("agent.title")}
              subtitle={
                isFree
                  ? t("workz.freeSubtitle", { dir: freeTaskWorkspace ?? "…" })
                  : t("agent.subtitle", {
                      project: projectDir || t("agent.openProjectFallback"),
                    })
              }
              onSelectScenario={onSelectScenario}
            />
          )}
          {steps.map((m, i) => {
            const isStreamingLast = m.role === "assistant" && busy && i === steps.length - 1;
            return (
              <div key={m.id ?? `step-${i}`} className={`agentz-workz-msg ${m.role}`}>
                <div className="agentz-workz-msg-role">
                  {m.role === "user" ? t("chat.you") : t("agent.role")}
                </div>
                <div className="agentz-workz-msg-body">
                  {m.role === "assistant" && m.tools.length > 0 ? (
                    interleaveTools(m.text, m.tools).map((seg, si) => (
                      <div key={`seg-${si}`}>
                        {seg.text && (
                          <div className="agentz-workz-msg-bubble">
                            <Markdown content={seg.text} />
                          </div>
                        )}
                        {seg.tools.length > 0 && <ToolTrace items={seg.tools} />}
                      </div>
                    ))
                  ) : m.text ? (
                    <div className="agentz-workz-msg-bubble">
                      {m.role === "assistant" ? (
                        <Markdown content={m.text} />
                      ) : (
                        <div className="agentz-workz-msg-text">{m.text}</div>
                      )}
                    </div>
                  ) : isStreamingLast ? (
                    <div className="agentz-workz-msg-bubble agentz-workz-msg-thinking">
                      {t("agent.working")}
                    </div>
                  ) : null}
                  {m.role === "assistant" && m.id && !busy && onForkCheckpoint && onRestoreCheckpoint && (
                    <div className="agentz-checkpoint">
                      <span className="agentz-checkpoint-label">{t("chat.checkpoint")}</span>
                      <ForkMenu onFork={(fullHistory) => void onForkCheckpoint(m.id!, fullHistory)} />
                      <button
                        type="button"
                        data-testid={`workz-restore-${m.id}`}
                        className="agentz-checkpoint-btn muted"
                        onClick={() => void onRestoreCheckpoint(m.id!)}
                        title={t("chat.checkpointRestore")}
                      >
                        {t("chat.checkpointRestore")}
                      </button>
                    </div>
                  )}
                  {m.role === "assistant" && m.turnId && !isStreamingLast && projectDir && (
                    <div className="agentz-workz-turn-actions">
                      <button
                        type="button"
                        data-testid={`workz-undo-turn-${m.turnId}`}
                        className="agentz-workz-turn-undo-btn"
                        disabled={busy}
                        onClick={async () => {
                          const sid = sessionId ?? selectedId;
                          if (!sid || !projectDir || !m.turnId) return;
                          if (!(await confirmDialog(t("agent.rollbackTurnConfirm"), { title: t("agent.rollbackTurn") }))) return;
                          try {
                            const restored = await journalUndoTurn(projectDir, sid, m.turnId);
                            onRefreshGitChanges();
                            onShowToast(t("agent.rollbackTurnOk", { count: restored.length }));
                          } catch (e) {
                            onSetError(String(e));
                          }
                        }}
                      >
                        {t("agent.rollbackTurn")}
                      </button>
                    </div>
                  )}
                </div>
              </div>
            );
          })}
          {onInterruptQueued && onSteerQueued && onEditQueued && onRemoveQueued && (
            <WorkzQueuedList
              items={queued}
              onInterrupt={onInterruptQueued}
              onSteer={onSteerQueued}
              onEdit={onEditQueued}
              onRemove={onRemoveQueued}
            />
          )}
          {bottomCards.map((card) => (
            <div key={card.requestId} className="agentz-workz-msg assistant">
              <div className="agentz-workz-msg-role">{t("agent.role")}</div>
              <div className="agentz-workz-msg-body">
                <InteractiveCard
                  requestId={card.requestId}
                  uiDefinition={card.uiDefinition}
                  listenOpen={card.listenOpen}
                  wizardStepHint={card.wizardStepHint}
                  onSubmitted={(values) => onMarkCardSubmitted(card.requestId, values)}
                  onActionSent={() => onMarkCardActionSent(card.requestId)}
                />
              </div>
            </div>
          ))}
          {permissionRequest && (
            <div className="agentz-workz-msg assistant">
              <div className="agentz-workz-msg-role">{t("agent.role")}</div>
              <div className="agentz-workz-msg-body">
                <PermissionCard request={permissionRequest} onResolved={onClearPermission} />
              </div>
            </div>
          )}
        </div>
      )}
    </div>
    </InteractiveCardsContext.Provider>
  );
}
