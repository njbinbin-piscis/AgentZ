import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { useExtensionUi } from "../../extensions/ui/useExtensionUi";
import { extensionService } from "../../extensions/extensionService";
import { extensionUiStore } from "../../extensions/extensionUiStore";
import { describeRemoteDir } from "../../extensions/remoteTargets";
import { useRemotePortWatch } from "../../extensions/remotePorts";
import {
  getUsageSummary,
  onChatEvent,
  type AgentEvent,
  type ChatEventEnvelope,
  type UsageSummary,
} from "../../services/tauri/chat";
import { getActiveChatSession, subscribeActiveChatSession } from "../../services/activeChatSession";
import { useAppSettings } from "../../hooks/useAppSettings";
import { useBackgroundTasks } from "../../hooks/useBackgroundTasks";
import { loadScopedModelId, saveScopedModelId } from "../../utils/modelPrefs";
import { summarizeUsage, type UsageSummaryView } from "../../utils/modelPricing";
import type { BottomTab } from "./BottomPanel";

interface IdeStatusBarProps {
  projectDir: string | null;
  onOpenPanel: (tab: BottomTab) => void;
  /** Open the Extensions sidebar (marketplace + enable/disable). */
  onOpenExtensions: () => void;
  /** Open the remote connection dialog. */
  onOpenRemote: () => void;
}

const CODICON_GLYPHS: Record<string, string> = {
  versions: "⧉",
  layers: "⧉",
  package: "⧉",
  play: "▶",
  "run-all": "▶",
  "debug-start": "▶",
  check: "✓",
  pass: "✓",
  verified: "✓",
  error: "✕",
  close: "✕",
  warning: "⚠",
  alert: "⚠",
  info: "ℹ",
  sync: "↻",
  loading: "↻",
  refresh: "↻",
  gear: "⚙",
  "settings-gear": "⚙",
  tools: "⚙",
  terminal: ">_",
  console: ">_",
  "cloud-download": "↓",
  download: "↓",
  "arrow-down": "↓",
  add: "+",
  plus: "+",
  trash: "🗑",
  trashcan: "🗑",
  "git-branch": "⑂",
  "source-control": "⑂",
  search: "⌕",
  zap: "⚡",
  flame: "⚡",
  rocket: "⚡",
};

/** Convert VS Code `$(codicon)` tokens and uninterpolated `$Version` placeholders into clean display text. */
export function formatCodiconText(raw: string): string {
  if (!raw) return "";
  return raw
    .replace(/\$\(([a-zA-Z0-9_-]+)(?:~[a-zA-Z0-9_-]+)?\)/g, (_m, name: string) => {
      const glyph = CODICON_GLYPHS[name.toLowerCase()];
      return glyph ?? "";
    })
    .replace(/\$\{?Version\}?/gi, "")
    .replace(/\s{2,}/g, " ")
    .trim();
}

export function formatCompactTokens(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`;
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}k`;
  return String(tokens);
}

/**
 * Full-width application status bar. The extension indicator always opens
 * the Extensions sidebar (marketplace + installed extensions); extension
 * output lives in the bottom panel and is opened from there, not from here.
 */
export default function IdeStatusBar({ projectDir, onOpenPanel, onOpenExtensions, onOpenRemote }: IdeStatusBarProps) {
  const { t } = useTranslation();
  const bg = useBackgroundTasks(projectDir);
  const { statusBar, running, scm, hostError } = useExtensionUi();
  const { appSettings, llmProviders, defaultModelLabel } = useAppSettings();
  const [selectedModelId, setSelectedModelId] = useState<string>(() => loadScopedModelId("codez"));
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  const sessionId = useSyncExternalStore(subscribeActiveChatSession, getActiveChatSession);

  // Usage is persisted by the kernel per (session, model); reload on session or
  // project change and after each model response / finished run.
  useEffect(() => {
    if (!projectDir) {
      setSummary(null);
      return;
    }
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = () => {
      getUsageSummary(sessionId, projectDir)
        .then((s) => alive && setSummary(s))
        .catch(() => {});
    };
    load();
    let unlisten: (() => void) | undefined;
    onChatEvent((env: ChatEventEnvelope) => {
      const isUsage = env.channel === "agent_event" && (env.payload as AgentEvent).type === "context_usage";
      if (!isUsage && env.channel !== "agent_final") return;
      clearTimeout(timer);
      timer = setTimeout(load, 400);
    })
      .then((fn) => {
        if (alive) unlisten = fn;
        else fn();
      })
      .catch(() => {});
    return () => {
      alive = false;
      clearTimeout(timer);
      unlisten?.();
    };
  }, [projectDir, sessionId]);

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

  const sessionUsage = summarizeUsage(summary?.session ?? [], appSettings?.model_prices);
  const projectUsage = summarizeUsage(summary?.project ?? [], appSettings?.model_prices);
  const usageText = (u: UsageSummaryView) =>
    `${formatCompactTokens(u.inputTokens + u.outputTokens)} tok${u.cost ? ` ? ${u.cost}${u.partial ? "+" : ""}` : ""}`;
  const usageDetail = (label: string, u: UsageSummaryView) =>
    `${label}: ${t("chat.usageInput")} ${u.inputTokens} ? ${t("chat.usageOutput")} ${u.outputTokens}${u.cost ? ` ? ${u.cost}` : ""}`;

  const left = statusBar.filter((s) => s.alignment === 1);
  const right = statusBar.filter((s) => s.alignment === 2);

  const renderItem = (entry: (typeof statusBar)[number]) => {
    const label = formatCodiconText(entry.text);
    const tip = formatCodiconText(entry.tooltip ?? entry.name ?? "");
    return (
      <button
        key={entry.id}
        type="button"
        className="ide-status-item"
        title={tip || undefined}
        style={entry.color ? { color: entry.color } : undefined}
        onClick={() =>
          entry.command && void extensionService.executeCommand(entry.command, entry.arguments ?? [])
        }
      >
        {label}
      </button>
    );
  };

  // The [Extensions] indicator must land on the Extensions sidebar tab in every
  // host state. Routing it to the bottom "output" panel when running was a
  // dead end: with no output channels that panel falls back to the terminal,
  // so the click appeared to open the terminal instead of the plugins page.
  // Starting the host stays available inside the Extensions sidebar itself.
  const handleExtClick = () => {
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
        type="button"
        data-testid="statusbar-extensions"
        className={`ide-status-item ide-status-ext ${running ? "running" : ""}`}
        title={running ? t("extensions.hostRunning") : t("extensions.hostOffHint")}
        onClick={handleExtClick}
      >
        <span className="ide-status-dot" />
        {running ? t("extensions.nav") : t("extensions.hostOff")}
      </button>

      {bg.running.length > 0 && (
        <span
          className="ide-status-item"
          title={bg.running.map((x) => x.label).join("\n")}
        >
          <span className="agentz-session-dot running" /> {t("chat.bgRunningCount", { count: bg.running.length })}
        </span>
      )}

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
        title={[
          usageDetail(t("chat.usageSession"), sessionUsage),
          usageDetail(t("chat.usageProject"), projectUsage),
          projectUsage.partial ? t("chat.costNoPrice") : "",
        ]
          .filter(Boolean)
          .join("\n")}
      >
        {t("chat.usageSession")} {usageText(sessionUsage)} | {t("chat.usageProject")} {usageText(projectUsage)}
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
