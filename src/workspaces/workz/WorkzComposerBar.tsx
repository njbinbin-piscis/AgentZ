import { useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";
import ChatComposer from "../../components/ChatComposer";
import DropdownSelect, { type DropdownOption } from "../../components/DropdownSelect";
import { CODEBASE_MENTION, GRAPH_MENTION } from "../../hooks/useAtMention";
import type { SlashCommandInfo } from "../../services/tauri/slashCommands";
import { agentTaskApi, type AgentTaskInfo } from "../../services/tauri/agentTask";
import type { ChatAttachment } from "../../services/tauri/chat";
import type { InstalledSkill } from "../../services/tauri/workbench";
import type { ConnectorInfo } from "../../services/tauri/connectors";
import type { TeamInfo } from "../../services/tauri/teams";
import type { AgentInfo } from "../../services/tauri/agents";
import type { WorkflowStatus } from "../../services/tauri/workflow";
import type { LibraryInitialState } from "../codez/resources/types";

export interface WorkzComposerBarProps {
  projectDir: string | null;
  isFree: boolean;
  freeTaskWorkspace: string | null;
  managedWorkspace: string | null;
  cwdOptions: DropdownOption[];
  onChangeCwd: (dir: string) => void;
  isolate: boolean;
  onChangeIsolate: (isolate: boolean) => void;
  taskBound: boolean;
  busy: boolean;
  startMode: "commit" | "working";
  onChangeStartMode: (mode: "commit" | "working") => void;
  startBase: string;
  onChangeStartBase: (base: string) => void;
  teams: TeamInfo[];
  activeTeam: string;
  selectedTeam?: TeamInfo;
  teamOptions: DropdownOption[];
  onChangeTeam: (teamId: string) => void;
  agents: AgentInfo[];
  activeAgentId: string;
  agentOptions: DropdownOption[];
  onChangeAgent: (agentId: string) => void;
  isSwarmTeam: boolean;
  activePoolId: string | null;
  onOpenBoard: () => void;
  isWorkflowTeam: boolean;
  workflowRunId: string | null;
  workflowStatus: WorkflowStatus | null;
  onOpenWorkflow: () => void;
  onOpenRuns: () => void;
  worktree: AgentTaskInfo | null;
  onOpenReview: (wt: AgentTaskInfo) => void;
  selectedId: string | null;
  hasSteps: boolean;
  onShowToast: (msg: string) => void;
  onSetError: (err: string) => void;
  goal: string;
  onGoalChange: (value: string, caret?: number) => void;
  onRun: () => void;
  onStop: () => void;
  canSend: boolean;
  taRef: RefObject<HTMLTextAreaElement>;
  onKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  onPaste: (e: React.ClipboardEvent<HTMLTextAreaElement>) => void;
  modelId: string;
  modelOptions: DropdownOption[];
  onModelChange: (id: string) => void;
  showModelSelector: boolean;
  showGenericComposerTools: boolean;
  installedSkills: InstalledSkill[];
  enabledSkills: string[];
  onChangeEnabledSkills: (skills: string[]) => void;
  connectors: ConnectorInfo[];
  enabledConnectors: string[];
  onChangeEnabledConnectors: (connectors: string[]) => void;
  onOpenLibrary?: (initial?: LibraryInitialState) => void;
  mention: { active: number } | null;
  mentionMatches: string[];
  onPickMention: (path: string) => void;
  slash: { active: number } | null;
  slashMatches: SlashCommandInfo[];
  onPickSlash: (cmd: SlashCommandInfo) => void;
  slashLabel: (cmd: SlashCommandInfo) => string;
  composerModeNotice: string | null;
  attachment: ChatAttachment | null;
  attachmentPreview: string | null;
  onAttach: () => void;
  onClearAttachment: () => void;
}

export default function WorkzComposerBar({
  projectDir,
  isFree,
  freeTaskWorkspace,
  managedWorkspace,
  cwdOptions,
  onChangeCwd,
  isolate,
  onChangeIsolate,
  taskBound,
  busy,
  startMode,
  onChangeStartMode,
  startBase,
  onChangeStartBase,
  teams,
  activeTeam,
  selectedTeam,
  teamOptions,
  onChangeTeam,
  agents,
  activeAgentId,
  agentOptions,
  onChangeAgent,
  isSwarmTeam,
  activePoolId,
  onOpenBoard,
  isWorkflowTeam,
  workflowRunId,
  workflowStatus,
  onOpenWorkflow,
  onOpenRuns,
  worktree,
  onOpenReview,
  selectedId,
  hasSteps,
  onShowToast,
  onSetError,
  goal,
  onGoalChange,
  onRun,
  onStop,
  canSend,
  taRef,
  onKeyDown,
  onPaste,
  modelId,
  modelOptions,
  onModelChange,
  showModelSelector,
  showGenericComposerTools,
  installedSkills,
  enabledSkills,
  onChangeEnabledSkills,
  connectors,
  enabledConnectors,
  onChangeEnabledConnectors,
  onOpenLibrary,
  mention,
  mentionMatches,
  onPickMention,
  slash,
  slashMatches,
  onPickSlash,
  slashLabel,
  composerModeNotice,
  attachment,
  attachmentPreview,
  onAttach,
  onClearAttachment,
}: WorkzComposerBarProps) {
  const { t, i18n } = useTranslation();
  const [sandboxAdvancedOpen, setSandboxAdvancedOpen] = useState(false);
  const [betaSwarmExpanded, setBetaSwarmExpanded] = useState(false);

  return (
    <>
      <div className="agentz-workz-isolate-bar">
        <div className="agentz-workz-bar-left">
          {isFree ? (
            <div className="agentz-workz-pill-menu">
              <span className="agentz-workz-pill-label" title={t("workz.cwdTitle")}>
                {t("workz.cwdLabel")}
              </span>
              <DropdownSelect
                variant="pill"
                placement="up"
                value={freeTaskWorkspace ?? ""}
                options={cwdOptions}
                disabled={busy || !!managedWorkspace}
                title={t("workz.cwdTitle")}
                onChange={onChangeCwd}
              />
            </div>
          ) : (
            <label
              className="agentz-workz-isolate-toggle"
              title={taskBound ? t("agent.modeLocked") : t("agent.isolateHint")}
            >
              <input
                type="checkbox"
                checked={isolate}
                disabled={busy || taskBound}
                onChange={(e) => onChangeIsolate(e.target.checked)}
              />
              <span>{isolate ? t("agent.isolateOn") : t("agent.isolateOff")}</span>
            </label>
          )}

          {isolate && !isFree && !taskBound && (
            <div className="agentz-workz-pill-menu">
              <button
                type="button"
                data-testid="workz-sandbox-advanced-btn"
                className="agentz-workz-pill-btn"
                disabled={busy}
                title={t("agent.startSourceHint")}
                onClick={() => setSandboxAdvancedOpen((prev) => !prev)}
              >
                {t("workz.sandboxAdvancedToggle")}
              </button>
              {sandboxAdvancedOpen && (
                <>
                  <DropdownSelect
                    variant="pill"
                    placement="up"
                    value={startMode}
                    disabled={busy}
                    title={t("agent.startSourceHint")}
                    options={[
                      { id: "working", label: t("agent.startWorking") },
                      { id: "commit", label: t("agent.startCommit") },
                    ]}
                    onChange={(value) => onChangeStartMode(value as "commit" | "working")}
                  />
                  {startMode === "commit" && (
                    <input
                      aria-label={t("agent.startReference")}
                      placeholder="HEAD"
                      value={startBase}
                      disabled={busy}
                      onChange={(event) => onChangeStartBase(event.target.value)}
                      style={{ width: 120 }}
                    />
                  )}
                </>
              )}
            </div>
          )}

          {teams.length > 0 && (
            <>
              <button
                type="button"
                data-testid="workz-beta-swarm-toggle"
                className="agentz-workz-pill-btn agentz-workz-beta-toggle"
                onClick={() => setBetaSwarmExpanded((prev) => !prev)}
              >
                {t("workz.betaAdvancedToggle")}
              </button>
              {(betaSwarmExpanded || Boolean(activeTeam)) && (
                <div
                  className={`agentz-workz-pill-menu${taskBound ? " is-locked" : ""}`}
                >
                  <span
                    className="agentz-workz-pill-label"
                    title={
                      taskBound
                        ? t("agent.modeLocked")
                        : activeTeam
                          ? selectedTeam?.mode === "workflow"
                            ? t("agent.teamHintWorkflow")
                            : t("agent.teamHintSwarm")
                          : t("agent.teamHint")
                    }
                  >
                    {t("agent.team")} (Beta)
                  </span>
                  <DropdownSelect
                    variant="pill"
                    placement="up"
                    value={activeTeam}
                    options={teamOptions}
                    disabled={busy || taskBound}
                    title={
                      taskBound
                        ? t("agent.modeLocked")
                        : activeTeam
                          ? selectedTeam?.mode === "workflow"
                            ? t("agent.teamHintWorkflow")
                            : t("agent.teamHintSwarm")
                          : t("agent.teamHint")
                    }
                    onChange={onChangeTeam}
                  />
                </div>
              )}
            </>
          )}

          {!activeTeam && agents.length > 0 && (
            <div
              className={`agentz-workz-pill-menu${taskBound ? " is-locked" : ""}`}
            >
              <span
                className="agentz-workz-pill-label"
                title={taskBound ? t("agent.modeLocked") : t("agent.agentHint")}
              >
                {t("agent.agentLabel")}
              </span>
              <DropdownSelect
                variant="pill"
                placement="up"
                value={activeAgentId}
                options={agentOptions}
                disabled={busy || taskBound}
                title={taskBound ? t("agent.modeLocked") : t("agent.agentHint")}
                onChange={onChangeAgent}
              />
            </div>
          )}
        </div>

        <div className="agentz-workz-bar-right">
          {isSwarmTeam && activePoolId && (
            <button
              type="button"
              className="agentz-workz-review-btn"
              onClick={onOpenBoard}
            >
              {t("collab.openBoard")}
            </button>
          )}
          {isWorkflowTeam && workflowRunId && (
            <button
              type="button"
              className="agentz-workz-review-btn"
              onClick={onOpenWorkflow}
            >
              {t("workflow.designer")}
              {workflowStatus ? ` · ${t(`workflow.status.${workflowStatus}`)}` : ""}
            </button>
          )}
          {isWorkflowTeam && (
            <button
              type="button"
              className="agentz-workz-review-btn"
              onClick={onOpenRuns}
            >
              {t("workflow.openHistory")}
            </button>
          )}
          {worktree && (
            <button
              type="button"
              className="agentz-workz-review-btn"
              onClick={() => onOpenReview(worktree)}
              disabled={busy}
              title={worktree.branch}
            >
              {t("agent.review")}
            </button>
          )}
          {!worktree && selectedId && projectDir && hasSteps && (
            <button
              type="button"
              className="agentz-workz-review-btn"
              data-testid="workz-rollback-snapshot-btn"
              disabled={busy}
              onClick={async () => {
                try {
                  const restored = await agentTaskApi.rollbackSnapshot(projectDir, selectedId);
                  onShowToast(t("agent.rollbackSnapshotOk", { count: restored.length }));
                } catch (e) {
                  onSetError(String(e));
                }
              }}
            >
              {t("agent.rollbackSnapshot")}
            </button>
          )}
        </div>
      </div>

      <ChatComposer
        value={goal}
        onChange={onGoalChange}
        onSubmit={onRun}
        onStop={onStop}
        busy={busy}
        placeholder={
          !projectDir
            ? t("agent.noProject")
            : busy
              ? t("agent.placeholderBusy")
              : t("agent.placeholder")
        }
        canSend={canSend}
        inputDisabled={!projectDir}
        textareaRef={taRef}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
        modelId={modelId}
        modelOptions={modelOptions}
        onModelChange={onModelChange}
        showModelSelector={showModelSelector}
        skillSelector={
          showGenericComposerTools
            ? {
                label: t("chat.skills"),
                emptyHint: t("chat.skillsEmpty"),
                onEmptyHintClick: onOpenLibrary
                  ? () => onOpenLibrary({ category: "skill", view: "discover" })
                  : undefined,
                selected: enabledSkills,
                onChange: onChangeEnabledSkills,
                hintPresentation: "toast",
                onHintToast: onShowToast,
                options: installedSkills.map((s) => ({
                  id: s.slug,
                  label: s.name,
                  hint:
                    i18n.language.startsWith("zh") && s.description_zh?.trim()
                      ? s.description_zh
                      : s.description,
                })),
              }
            : undefined
        }
        mentionPopup={
          mention && mentionMatches.length > 0 ? (
            <div className="agentz-mention-popup">
              {mentionMatches.map((path, i) => (
                <div
                  key={path}
                  className={`agentz-mention-item ${i === mention.active ? "active" : ""}`}
                  onMouseDown={(ev) => {
                    ev.preventDefault();
                    onPickMention(path);
                  }}
                >
                  {path === CODEBASE_MENTION
                    ? `codebase · ${t("chat.mentionCodebase")}`
                    : path === GRAPH_MENTION
                      ? `graph · ${t("chat.mentionGraph")}`
                      : path}
                </div>
              ))}
            </div>
          ) : slash && slashMatches.length > 0 ? (
            <div className="agentz-mention-popup">
              {slashMatches.map((cmd, i) => (
                <div
                  key={cmd.id}
                  className={`agentz-mention-item ${i === slash.active ? "active" : ""}`}
                  onMouseDown={(ev) => {
                    ev.preventDefault();
                    onPickSlash(cmd);
                  }}
                >
                  {slashLabel(cmd)}
                </div>
              ))}
            </div>
          ) : undefined
        }
        connectorSelector={
          showGenericComposerTools
            ? {
                label: t("chat.connectors"),
                emptyHint: t("chat.connectorsEmpty"),
                onEmptyHintClick: onOpenLibrary
                  ? () => onOpenLibrary({ category: "connector", view: "discover" })
                  : undefined,
                selected: enabledConnectors,
                onChange: onChangeEnabledConnectors,
                options: connectors.map((c) => ({
                  id: c.id,
                  label: `${c.icon ? `${c.icon} ` : ""}${c.name}`,
                  hint: [
                    c.category,
                    c.authorized ? undefined : t("studio.connectorUnauthorized"),
                    c.description,
                  ]
                    .filter(Boolean)
                    .join(" · "),
                })),
              }
            : undefined
        }
        modeNotice={composerModeNotice}
        attachment={attachment}
        attachmentPreview={attachmentPreview}
        onAttach={onAttach}
        onClearAttachment={onClearAttachment}
        attachTitle={t("chat.attachFile")}
        removeAttachmentTitle={t("chat.removeAttachment")}
        stopTitle={t("agent.stop")}
        sendTitle={t("agent.run")}
      />
    </>
  );
}
