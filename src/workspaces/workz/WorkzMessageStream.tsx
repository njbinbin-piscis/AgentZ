import type { RefObject } from "react";
import { useTranslation } from "react-i18next";
import { journalUndoTurn } from "../../services/tauri/chat";
import PermissionCard, { type PermissionRequestCard } from "../../components/chat/PermissionCard";
import InteractiveCard from "../../components/chat/InteractiveCard";
import type { InteractiveCardState } from "../../hooks/useInteractiveCards";
import ToolTrace, { interleaveTools } from "../../components/ToolTrace";
import Markdown from "../codez/Markdown";
import PoolActivityFeed from "./PoolActivityFeed";
import WorkzEmptyState, { type WorkzStarterScenario } from "./WorkzEmptyState";
import type { AgentStep } from "./agentArtifacts";

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
  permissionRequest: PermissionRequestCard | null;
  onClearPermission: () => void;
  onMarkCardSubmitted: (requestId: string) => void;
  onMarkCardActionSent: (requestId: string) => void;
  onSelectScenario: (scenario: WorkzStarterScenario) => void;
  onRefreshGitChanges: () => void;
  onShowToast: (msg: string) => void;
  onSetError: (err: string) => void;
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
  permissionRequest,
  onClearPermission,
  onMarkCardSubmitted,
  onMarkCardActionSent,
  onSelectScenario,
  onRefreshGitChanges,
  onShowToast,
  onSetError,
}: WorkzMessageStreamProps) {
  const { t } = useTranslation();

  return (
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
          {pendingCards.map((card) => (
            <div key={card.requestId} className="agentz-workz-msg assistant">
              <div className="agentz-workz-msg-role">{t("agent.role")}</div>
              <div className="agentz-workz-msg-body">
                <InteractiveCard
                  requestId={card.requestId}
                  uiDefinition={card.uiDefinition}
                  listenOpen={card.listenOpen}
                  wizardStepHint={card.wizardStepHint}
                  onSubmitted={() => onMarkCardSubmitted(card.requestId)}
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
  );
}
