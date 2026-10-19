import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useExtensionUi } from "../../extensions/ui/useExtensionUi";
import { extensionService } from "../../extensions/extensionService";
import { extensionUiStore } from "../../extensions/extensionUiStore";
import { describeRemoteDir } from "../../extensions/remoteTargets";
import { useRemotePortWatch } from "../../extensions/remotePorts";
import { onChatEvent, type AgentEvent, type ChatEventEnvelope } from "../../services/tauri/chat";
import { useAppSettings } from "../../hooks/useAppSettings";
import { loadScopedModelId, saveScopedModelId } from "../../utils/modelPrefs";
import type { BottomTab } from "./BottomPanel";

interface IdeStatusBarProps {
  projectDir: string | null;
  onOpenPanel: (tab: BottomTab) => void;
  /** Open the Extensions sidebar (marketplace + enable/disable). */
  onOpenExtensions: () => void;
  /** Open the remote connection dialog. */
  onOpenRemote: () => void;
}

export interface StatusBarUsageStats {
  inputTokens: number;
  outputTokens: number;
}

/** Rough cost estimation ($3 / 1M input + $15 / 1M output as standard reference). */
export function estimateTokenCostUsd(inputTokens: number, outputTokens: number): string {
  const totalUsd = (inputTokens * 3 + outputTokens * 15) / 1_000_000;
  if (totalUsd < 0.001 && inputTokens + outputTokens > 0) return "<$0.001";
  return `$${totalUsd.toFixed(3)}`;
}

export function formatCompactTokens(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}k`;
  return String(tokens);
}

/**
 * Full-width application status bar. Extension indicator opens the marketplace
 * when the host is off, or extension output when running.
 */
export default function IdeStatusBar({ projectDir, onOpenPanel, onOpenExtensions, onOpenRemote }: IdeStatusBarProps) {
  const { t } = useTranslation();
  const { statusBar, running, scm, hostError } = useExtensionUi();
  const { llmProviders, defaultModelLabel } = useAppSettings();
  const [selectedModelId, setSelectedModelId] = useState<string>(() => loadScopedModelId("codez"));
  const [usage, setUsage] = useState<StatusBarUsageStats>({ inputTokens: 0, outputTokens: 0 });

  useEffect(() => {
    setUsage({ inputTokens: 0, outputTokens: 0 });
  }, [projectDir]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    onChatEvent((env: ChatEventEnvelope) => {
      if (env.channel !== "agent_event") return;
      const evt = env.payload as AgentEvent;
      if (evt.type === "context_usage") {
        setUsage({
          inputTokens: evt.cumulative_input_tokens ?? evt.estimated_input_tokens ?? 0,
          outputTokens: evt.cumulative_output_tokens ?? 0,
        });
      }
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const modelOptions = useMemo(() => {
    const baseLabel = defaultModelLabel || t("chat.defaultModel");
    const items = [{ id: "", label: baseLabel }];
    for (const p of llmProviders) {
      items.push({
        id: p.id,
        label: p.label || `${p.provider}/${p.model}`,
      });
    }
    return items;
  }, [llmProviders, defaultModelLabel, t]);

  const handleCycleModel = () => {
    if (modelOptions.length <= 1) return;
    const idx = modelOptions.findIndex((m) => m.id === selectedModelId);
    const next = modelOptions[(idx + 1) % modelOptions.length];
    setSelectedModelId(next.id);
    saveScopedModelId("codez", next.id);
  };

  const activeModelLabel =
    modelOptions.find((m) => m.id === selectedModelId)?.label ??
    defaultModelLabel ??
    t("chat.defaultModel");

  const totalTokens = usage.inputTokens + usage.outputTokens;
  const costLabel = estimateTokenCostUsd(usage.inputTokens, usage.outputTokens);

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

      <button
        type="button"
        data-testid="statusbar-model-switcher"
        className="ide-status-item ide-status-model"
        title={t("chat.modelTitle")}
        onClick={handleCycleModel}
      >
        ✦ {activeModelLabel}
      </button>

      <span
        data-testid="statusbar-token-cost-pill"
        className="ide-status-item ide-status-usage"
        title={`Input: ${usage.inputTokens} · Output: ${usage.outputTokens}`}
      >
        ◎ {formatCompactTokens(totalTokens)} tok · {costLabel}
      </span>

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
