import type { PickedElement } from "../../services/tauri/browser";
import type { ChatAttachment } from "../../services/tauri/chat";
import ChatComposer from "../../components/ChatComposer";
import TaskPanel from "../../components/TaskPanel";
import ContextUsageRing from "../../components/ContextUsageRing";
import AssistantMessageList from "./AssistantMessageList";
import BackgroundTasksPanel from "./BackgroundTasksPanel";
import { useBackgroundTasks } from "../../hooks/useBackgroundTasks";
import { useAssistantSession } from "./useAssistantSession";
import "./AssistantPanel.css";

export interface AssistantPanelProps {
  projectDir: string | null;
  onOpenFolder: () => void;
  /** External request to insert @file references into the composer. */
  insertRequest?: { paths: string[]; nonce: number } | null;
  /** External request to add a picked browser element chip to the composer. */
  insertElementRequest?: { element: PickedElement; nonce: number } | null;
  /** External request to add a terminal selection chip to the composer. */
  insertTerminalRequest?: { snippetId: string; text: string; nonce: number } | null;
  /** External request to set an attachment (e.g. a browser screenshot). */
  attachRequest?: { attachment: ChatAttachment; preview: string | null; nonce: number } | null;
  /** Called after an external attachRequest has been applied to the composer. */
  onAttachRequestHandled?: () => void;
}

export default function AssistantPanel(props: AssistantPanelProps) {
  const { projectDir, onOpenFolder } = props;
  const s = useAssistantSession(props);
  const bg = useBackgroundTasks(projectDir);

  return (
    <div className="agentz-assistant" ref={s.panelRef}>
      <div className="agentz-assistant-header">
        <span>{s.t("chat.title")}</span>
        <div className="agentz-assistant-actions">
          <ContextUsageRing usage={s.contextUsage} />
          <button
            className={s.showSessions ? "active" : ""}
            onClick={() => s.setShowSessions((v) => !v)}
            title={s.t("chat.sessions")}
          >
            ☰
          </button>
          <button onClick={s.newSession} title={s.t("chat.newChat")}>
            ＋
          </button>
          <button
            onClick={() => void s.fork()}
            disabled={!s.sessionRef.current}
            title={s.t("chat.fork")}
          >
            ⑂
          </button>
          <button
            onClick={() => void s.exportCurrent()}
            disabled={!s.sessionRef.current}
            title={s.t("chat.exportSession")}
          >
            ⇪
          </button>
          <button
            onClick={() => void s.importFromFile()}
            disabled={!projectDir}
            title={s.t("chat.importSession")}
          >
            ⇩
          </button>
        </div>
      </div>

      {s.toast && <div className="agentz-assistant-toast">{s.toast}</div>}

      {s.dragOver && (
        <div className="agentz-assistant-dropzone">
          <span>{s.t("chat.dropToAddRefs")}</span>
        </div>
      )}

      {s.showSessions && (
        <div className="agentz-session-list">
          {s.sessions.length === 0 && (
            <div className="agentz-session-empty">{s.t("chat.noSessions")}</div>
          )}
          {s.sessions.map((sess) => (
            <div
              key={sess.id}
              className={`agentz-session-row ${sess.id === s.sessionRef.current ? "active" : ""}`}
              onClick={() => void s.switchSession(sess.id, sess.source)}
            >
              <span
                className={`agentz-session-dot ${
                  s.runningIds.includes(sess.id) || bg.runningSessionIds.includes(sess.id)
                    ? "running"
                    : // Persisted `running` without a live turn is stale; never pulse on it.
                      sess.status === "running"
                      ? "idle"
                      : sess.status
                }`}
              />
              <span className="agentz-session-title">
                {sess.title || s.t("chat.untitled")}
              </span>
              <span className="agentz-session-count">{sess.message_count}</span>
              <button
                className="agentz-session-del"
                title={s.t("chat.delete")}
                onClick={(e) => {
                  e.stopPropagation();
                  void s.removeSession(sess.id);
                }}
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}

      <BackgroundTasksPanel
        tasks={bg.tasks}
        currentSessionId={s.sessionRef.current}
        onKill={(id) => void bg.kill(id)}
        onOpenSession={(id) => {
          const meta = s.sessions.find((x) => x.id === id);
          void s.switchSession(id, meta?.source);
        }}
      />

      <TaskPanel
        planItems={s.planItems}
        busy={s.busy}
        open={s.taskPanelOpen}
        onOpenChange={s.setTaskPanelOpen}
        onClearPlan={() => s.setPlanItems([])}
      />

      <AssistantMessageList
        messages={s.messages}
        toolSteps={s.toolSteps}
        turnDiffsByTurnId={s.turnDiffsByTurnId}
        busy={s.busy}
        queuedView={s.queuedView}
        onRemoveQueued={s.removeQueued}
        pendingCards={s.pendingCards}
        scrollRef={s.scrollRef}
        hasMoreOlder={s.hasMoreOlder}
        onLoadOlder={s.loadOlderHistory}
        onSelectPath={s.onSelectPath}
        onForkCheckpoint={s.forkFromCheckpoint}
        onRestoreCheckpoint={s.restoreToCheckpoint}
        onUndoTurn={s.undoTurnChanges}
        onCardSubmitted={s.markSubmitted}
        onCardActionSent={s.markActionSent}
        onPlanModeChange={s.onModeChange}
        onPlanBuild={s.onPlanBuild}
        permissionRequest={s.permissionRequest}
        onPermissionResolved={s.handlePermissionResolved}
      />

      {s.messages.length === 0 && s.needsSetup && (
        <div className="agentz-assistant-setup-hint" role="note">
          <span>{s.t("chat.setupNeeded")}</span>
          <button
            type="button"
            className="agentz-assistant-error-action"
            onClick={s.requestOpenSettings}
          >
            {s.t("chat.firstRunOpenSettings")}
          </button>
        </div>
      )}

      {s.error && (
        <div className="agentz-assistant-error">
          {s.error}
          {s.setupError && (
            <>
              <div>{s.t("chat.setupErrorHint")}</div>
              <button
                type="button"
                className="agentz-assistant-error-action"
                onClick={s.requestOpenSettings}
              >
                {s.t("chat.firstRunOpenSettings")}
              </button>
            </>
          )}
          {!s.setupError && !s.firstTaskDone && s.firstTaskFailureHint(s.error) && (
            <>
              <div>{s.t(`chat.firstRunFailure${s.firstTaskFailureHint(s.error)}`)}</div>
              {s.firstTaskFailureHint(s.error) === "project" && onOpenFolder && (
                <button
                  type="button"
                  className="agentz-assistant-error-action"
                  onClick={onOpenFolder}
                >
                  {s.t("app.openFolder")}
                </button>
              )}
              {s.firstTaskFailureHint(s.error) === "connection" && (
                <button
                  type="button"
                  className="agentz-assistant-error-action"
                  onClick={s.requestOpenSettings}
                >
                  {s.t("chat.firstRunOpenSettings")}
                </button>
              )}
            </>
          )}
        </div>
      )}

      <ChatComposer
        value={s.input}
        onChange={s.onInputChange}
        onSubmit={s.submit}
        onStop={s.stopCurrent}
        busy={s.busy}
        placeholder={s.inputPlaceholder}
        canSend={s.canSend}
        inputDisabled={!projectDir}
        chips={s.composerChips}
        onRemoveChip={(id) =>
          s.setComposerChips((cur) => {
            const removed = cur.find((c) => c.id === id);
            s.composerDbg("chip removed", {
              id,
              removed: removed ? s.chipsSnapshot([removed])[0] : null,
            });
            if (removed?.kind === "image-attachment" && removed.preview.startsWith("blob:")) {
              URL.revokeObjectURL(removed.preview);
            }
            const next = cur.filter((c) => c.id !== id);
            s.composerDbg("composerChips after remove", s.chipsSnapshot(next));
            return next;
          })
        }
        textareaRef={s.taRef}
        onKeyDown={s.onKeyDown}
        onPaste={(e) => void s.handlePaste(e)}
        modelId={s.modelId}
        modelOptions={s.modelOptions}
        onModelChange={s.setModelId}
        attachment={null}
        attachmentPreview={null}
        onAttach={() => void s.handleAttach()}
        onClearAttachment={() => {}}
        attachTitle={s.t("chat.attachFile")}
        removeAttachmentTitle={s.t("chat.removeAttachment")}
        stopTitle={s.t("chat.stop")}
        sendTitle={s.t("chat.send")}
        modeSelector={{
          chatMode: s.chatMode,
          onChange: s.onModeChange,
          options: s.modeOptions,
        }}
        skillSelector={{
          label: s.t("chat.skills"),
          emptyHint: s.t("chat.skillsEmpty"),
          selected: s.enabledSkills,
          onChange: s.setEnabledSkills,
          hintPresentation: "toast",
          onHintToast: s.setToast,
          options: s.installedSkills.map((sk) => ({
            id: sk.slug,
            label: sk.name,
            hint:
              s.i18n.language.startsWith("zh") && sk.description_zh?.trim()
                ? sk.description_zh
                : sk.description,
          })),
        }}
        agentSelector={{
          value: s.activeAgent,
          onChange: s.setActiveAgent,
          options: [
            { id: "", label: s.t("chat.agentDefault") },
            ...s.agents.map((a) => ({
              id: a.id,
              label: a.icon ? `${a.icon} ${a.name}` : a.name,
              hint:
                (s.i18n.language.startsWith("zh") && a.description_zh?.trim()
                  ? a.description_zh
                  : a.description) || a.role,
            })),
          ],
        }}
        modeNotice={s.modeNotice}
        mentionPopup={
          s.slash && s.slashMatches.length > 0 ? (
            <div className="agentz-mention-popup">
              {s.slashMatches.map((cmd, i) => (
                <div
                  key={cmd.id}
                  className={`agentz-mention-item ${i === s.slash!.active ? "active" : ""}`}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    s.pickSlash(cmd);
                  }}
                >
                  {s.slashLabel(cmd)}
                </div>
              ))}
            </div>
          ) : s.mention && s.matches.length > 0 ? (
            <div className="agentz-mention-popup">
              {s.matches.map((f, i) => (
                <div
                  key={f}
                  className={`agentz-mention-item ${i === s.mention!.active ? "active" : ""}`}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    s.pickMention(f);
                  }}
                >
                  {f === s.CODEBASE_MENTION
                    ? `codebase · ${s.t("chat.mentionCodebase")}`
                    : f === s.GRAPH_MENTION
                      ? `graph · ${s.t("chat.mentionGraph")}`
                      : f}
                </div>
              ))}
            </div>
          ) : null
        }
      />
    </div>
  );
}
