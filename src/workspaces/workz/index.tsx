import TaskPanel from "../../components/TaskPanel";
import SessionSkillRevisions from "./SessionSkillRevisions";
import AgentTaskReview from "./AgentTaskReview";
import AgentFilePreview from "./AgentFilePreview";
import CollabBoard from "./CollabBoard";
import WorkflowRunPanel from "./WorkflowRunPanel";
import WorkflowRunsList from "./WorkflowRunsList";
import WorkzSidebar from "./WorkzSidebar";
import WorkzMessageStream from "./WorkzMessageStream";
import WorkzComposerBar from "./WorkzComposerBar";
import { useWorkzSessionStore, type WikiBuildAction } from "./useWorkzSessionStore";
import { useBackgroundTasks } from "../../hooks/useBackgroundTasks";
import type { LibraryInitialState } from "../codez/resources/types";
import "./Agent.css";

export type { WikiBuildAction };

export interface WorkZWorkspaceProps {
  projectDir: string | null;
  openTaskRequest?: { projectDir: string; taskId: string; nonce: number } | null;
  onOpenFolder: () => void;
  /** Switch the CodeZ project to `dir` and show CodeZ. */
  onOpenProjectInCodeZ?: (dir: string) => void;
  /** Handoff a task's isolated worktree (and optional file) to CodeZ with a return pill. */
  onOpenTaskWorktreeInCodeZ?: (opts: {
    taskId: string;
    worktreePath: string;
    filePath?: string | null;
  }) => void;
  /** Increment from title bar to trigger repo wiki generation. */
  wikiBuildNonce?: number;
  wikiBuildAction?: WikiBuildAction;
  onWikiBusyChange?: (busy: boolean) => void;
  onOpenLibrary?: (initial?: LibraryInitialState) => void;
}

export default function WorkZWorkspace(props: WorkZWorkspaceProps) {
  const {
    onOpenProjectInCodeZ,
    onOpenTaskWorktreeInCodeZ,
    onOpenLibrary,
  } = props;

  const s = useWorkzSessionStore(props);
  const bg = useBackgroundTasks(props.projectDir);
  const mergedRunningIds = [...new Set([...s.runningIds, ...bg.runningSessionIds])];

  return (
    <div className="agentz-agent" ref={s.panelRef}>
      {s.dragOver && (
        <div className="agentz-workz-dropzone">
          <span>{s.t("chat.dropToAttach")}</span>
        </div>
      )}
      {s.toast && <div className="agentz-workz-toast">{s.toast}</div>}
      <aside className="agentz-workz-sidebar">
        <WorkzSidebar
          overview={s.overview}
          activeDir={s.projectDir}
          activeTasks={s.tasks}
          groups={s.groups}
          groupErrors={s.groupErrors}
          collapsed={s.collapsed}
          selectedId={s.selectedId}
          runningIds={mergedRunningIds}
          onToggle={s.toggleGroup}
          onOpenSession={s.openSession}
          onNewSession={s.newSessionIn}
          onDeleteSession={s.deleteSessionIn}
          onAddProject={() => void s.addProject()}
          onRemoveProject={(dir) => void s.removeProject(dir)}
          onOpenInCodeZ={onOpenProjectInCodeZ}
        />
        <SessionSkillRevisions sessionId={s.selectedId} />
      </aside>

      <section className="agentz-workz-main">
        <div className={`agentz-workz-content${s.previewPath ? " has-preview" : ""}`}>
          <WorkzMessageStream
            projectDir={s.projectDir}
            isFree={s.isFree}
            freeTaskWorkspace={s.freeTaskWorkspace}
            isSwarmTeam={Boolean(s.isSwarmTeam)}
            swarmMainTab={s.swarmMainTab}
            onChangeSwarmTab={s.setSwarmMainTab}
            activePoolId={s.activePoolId}
            scrollRef={s.scrollRef}
            steps={s.steps}
            busy={s.busy}
            selectedId={s.selectedId}
            sessionId={s.sessionRef.current}
            pendingCards={s.pendingCards}
            permissionRequest={s.permissionRequest}
            onClearPermission={() => s.setPermissionRequest(null)}
            onMarkCardSubmitted={s.markSubmitted}
            onMarkCardActionSent={s.markActionSent}
            onSelectScenario={(scenario) => {
              s.setGoal(s.t(scenario.promptKey));
              if (!s.isFree && !s.taskBound && scenario.recommendIsolate) {
                s.setIsolate(true);
              }
              requestAnimationFrame(() => s.taRef.current?.focus());
            }}
            onRefreshGitChanges={s.refreshGitChanges}
            onShowToast={s.showToast}
            onSetError={s.setError}
          />
          {s.previewPath && s.projectDir && (
            <AgentFilePreview
              projectDir={s.projectDir}
              path={s.previewPath}
              workspaceDir={s.worktree?.worktree_path ?? (s.isFree ? s.freeTaskWorkspace : null)}
              onClose={() => s.setPreviewPath(null)}
              onOpenInCodeZ={
                onOpenTaskWorktreeInCodeZ
                  ? ({ worktreePath, filePath }) =>
                      onOpenTaskWorktreeInCodeZ({
                        taskId: s.selectedId ?? "preview",
                        worktreePath,
                        filePath,
                      })
                  : undefined
              }
              onContinueWithEdits={({ path: editedPath }) => {
                s.setGoal((prev) =>
                  prev.trim()
                    ? prev
                    : s.t("workz.continueWithEditsPrompt", { path: editedPath }),
                );
                s.taRef.current?.focus();
              }}
            />
          )}
        </div>

        {s.error && <div className="agentz-workz-error">{s.error}</div>}

        <TaskPanel
          className="agentz-workz-task-panel"
          planItems={s.planItems}
          busy={s.busy}
          open={s.taskPanelOpen}
          onOpenChange={s.setTaskPanelOpen}
          onClearPlan={() => s.setPlanItems([])}
        />

        <WorkzComposerBar
          projectDir={s.projectDir}
          isFree={s.isFree}
          freeTaskWorkspace={s.freeTaskWorkspace}
          managedWorkspace={s.managedWorkspace}
          cwdOptions={s.cwdOptions}
          onChangeCwd={(v) => void s.changeCwd(v)}
          isolate={s.isolate}
          onChangeIsolate={s.setIsolate}
          taskBound={s.taskBound}
          busy={s.busy}
          startMode={s.startMode}
          onChangeStartMode={s.setStartMode}
          startBase={s.startBase}
          onChangeStartBase={s.setStartBase}
          teams={s.teams}
          activeTeam={s.activeTeam}
          selectedTeam={s.selectedTeam}
          teamOptions={s.teamOptions}
          onChangeTeam={s.handleTeamChange}
          agents={s.agents}
          activeAgentId={s.activeAgentId}
          agentOptions={s.agentOptions}
          onChangeAgent={s.handleAgentChange}
          isSwarmTeam={Boolean(s.isSwarmTeam)}
          activePoolId={s.activePoolId}
          onOpenBoard={() => s.setBoardOpen(true)}
          isWorkflowTeam={Boolean(s.isWorkflowTeam)}
          workflowRunId={s.workflowRunId}
          workflowStatus={s.workflowStatus}
          onOpenWorkflow={() => s.setWorkflowOpen(true)}
          onOpenRuns={() => s.setRunsOpen(true)}
          worktree={s.worktree}
          onOpenReview={(wt) => s.setReviewTask(wt)}
          selectedId={s.selectedId}
          hasSteps={s.steps.length > 0}
          onShowToast={s.showToast}
          onSetError={s.setError}
          goal={s.goal}
          onGoalChange={(value, caret) => {
            s.setGoal(value);
            if (!s.detectAtMention(value, caret)) {
              s.detectSlash(value, caret);
            }
          }}
          onRun={s.run}
          onStop={s.stopActive}
          canSend={s.canSend}
          taRef={s.taRef}
          onKeyDown={s.onKeyDown}
          onPaste={(e) => void s.handlePaste(e)}
          modelId={s.modelId}
          modelOptions={s.modelOptions}
          onModelChange={s.setModelId}
          showModelSelector={s.showModelSelector}
          showGenericComposerTools={s.showGenericComposerTools}
          installedSkills={s.installedSkills}
          enabledSkills={s.enabledSkills}
          onChangeEnabledSkills={s.setEnabledSkills}
          connectors={s.connectors}
          enabledConnectors={s.enabledConnectors}
          onChangeEnabledConnectors={s.setEnabledConnectors}
          onOpenLibrary={onOpenLibrary}
          mention={s.mention}
          mentionMatches={s.mentionMatches}
          onPickMention={s.pickMention}
          slash={s.slash}
          slashMatches={s.slashMatches}
          onPickSlash={s.pickSlash}
          slashLabel={s.slashLabel}
          composerModeNotice={s.composerModeNotice}
          attachment={s.attachment}
          attachmentPreview={s.attachmentPreview}
          onAttach={() => void s.handleAttach()}
          onClearAttachment={s.clearAttachment}
        />
      </section>
      {s.boardOpen && s.activePoolId && s.projectDir && (
        <CollabBoard
          projectDir={s.projectDir}
          poolId={s.activePoolId}
          onClose={() => s.setBoardOpen(false)}
        />
      )}
      {s.workflowOpen && s.workflowRunId && (
        <WorkflowRunPanel
          runId={s.workflowRunId}
          onClose={() => s.setWorkflowOpen(false)}
          onRerun={(id) => {
            s.setWorkflowRunId(id);
            s.setWorkflowStatus("running");
          }}
        />
      )}
      {s.runsOpen && (
        <WorkflowRunsList
          teamId={s.activeTeam || null}
          onSelect={(id) => {
            s.setWorkflowRunId(id);
            s.setWorkflowStatus(null);
            s.setWorkflowOpen(true);
            s.setRunsOpen(false);
          }}
          onClose={() => s.setRunsOpen(false)}
        />
      )}
      {s.reviewTask && s.projectDir && (
        <AgentTaskReview
          projectDir={s.projectDir}
          task={s.reviewTask}
          onClose={() => s.setReviewTask(null)}
          onResolved={() => {
            s.refreshGitChanges();
            s.refreshTasks();
          }}
          onOpenInCodeZ={
            onOpenTaskWorktreeInCodeZ
              ? (opts) => {
                  s.setReviewTask(null);
                  onOpenTaskWorktreeInCodeZ(opts);
                }
              : undefined
          }
        />
      )}
    </div>
  );
}
