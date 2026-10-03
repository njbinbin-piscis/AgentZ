import { useTranslation } from "react-i18next";
import { useExtensionUi } from "../../extensions/ui/useExtensionUi";
import { extensionService } from "../../extensions/extensionService";
import { extensionUiStore } from "../../extensions/extensionUiStore";
import { describeRemoteDir } from "../../extensions/remoteTargets";
import { useRemotePortWatch } from "../../extensions/remotePorts";
import type { BottomTab } from "./BottomPanel";

interface IdeStatusBarProps {
  projectDir: string | null;
  onOpenPanel: (tab: BottomTab) => void;
  /** Open the Extensions sidebar (marketplace + enable/disable). */
  onOpenExtensions: () => void;
  /** Open the remote connection dialog. */
  onOpenRemote: () => void;
}

/**
 * Full-width application status bar. Extension indicator opens the marketplace
 * when the host is off, or extension output when running.
 */
export default function IdeStatusBar({ projectDir, onOpenPanel, onOpenExtensions, onOpenRemote }: IdeStatusBarProps) {
  const { t } = useTranslation();
  const { statusBar, running, scm, hostError } = useExtensionUi();
  const left = statusBar.filter((s) => s.alignment === 1);
  const right = statusBar.filter((s) => s.alignment === 2);

  const renderItem = (entry: (typeof statusBar)[number]) => (
    <button
      key={entry.id}
      className="ide-status-item"
      title={entry.tooltip}
      style={entry.color ? { color: entry.color } : undefined}
      onClick={() => entry.command && void extensionService.executeCommand(entry.command)}
    >
      {entry.text}
    </button>
  );

  const handleExtClick = () => {
    if (running) {
      onOpenPanel("output");
      return;
    }
    if (projectDir) {
      void extensionService.start(projectDir).then(() => {
        if (extensionService.isRunning) onOpenPanel("output");
        else onOpenExtensions();
      }).catch(() => onOpenExtensions());
      return;
    }
    onOpenExtensions();
  };

  const remoteLabel = projectDir ? describeRemoteDir(projectDir) : null;
  const forwards = useRemotePortWatch(!!remoteLabel && running);

  return (
    <div className="ide-status-bar">
      {remoteLabel ? (
        <button
          className={`ide-status-item ide-status-remote ${running ? "running" : ""}`}
          title={running ? t("remote.statusConnected") : t("remote.statusReconnect")}
          onClick={() => {
            if (!running && projectDir) void extensionService.start(projectDir, { force: true }).catch(() => undefined);
            else onOpenRemote();
          }}
        >
          {running ? "⇄" : "⚠"} {remoteLabel}
        </button>
      ) : (
        <button className="ide-status-item ide-status-remote-open" title={t("remote.dialog.open")} onClick={onOpenRemote}>
          ⇄ {t("remote.dialog.statusLabel")}
        </button>
      )}
      {forwards.length > 0 && (
        <button
          className="ide-status-item"
          title={forwards.map((f) => `${f.remote_port} → localhost:${f.local_port}`).join("\n")}
          onClick={onOpenExtensions}
        >
          {t("remote.ports.statusCount", { count: forwards.length })}
        </button>
      )}
      <button
        className={`ide-status-item ide-status-ext ${running ? "running" : ""}`}
        title={running ? t("extensions.hostRunning") : t("extensions.hostOffHint")}
        onClick={handleExtClick}
      >
        <span className="ide-status-dot" />
        {running ? t("extensions.nav") : t("extensions.hostOff")}
      </button>

      {scm.length > 0 && (
        <button className="ide-status-item" title={t("ide.sourceControl")} onClick={() => onOpenPanel("scm")}>
          ⑂ {scm.length}
        </button>
      )}

      {left.map(renderItem)}

      <div className="ide-status-spacer" />

      {right.map(renderItem)}

      {hostError && (
        <button
          className="ide-status-item ide-status-error"
          title={hostError}
          onClick={() => extensionUiStore.setHostError(null)}
        >
          ⚠ {t("extensions.hostError")}
        </button>
      )}
    </div>
  );
}
