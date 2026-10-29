import { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import {
  getSettings,
  saveSettingsWithSecrets,
  getFlashProvider,
  setFlashProvider,
  type LlmProviderConfig,
  type LlmSettings,
  type McpServerConfig,
  type SettingsResponse,
} from "../../services/tauri/settings";
import { setLanguage } from "../../i18n";
import { splitSettingsSecretUpdates } from "../../services/tauri/settingsSecrets";
import { notifySettingsRefresh } from "../../services/settingsRefresh";
import ExtensionsManager from "./ExtensionsManager";
import AssistantsTab from "./settings/AssistantsTab";
import RulesTab from "./settings/RulesTab";
import HooksTab from "./settings/HooksTab";
import WorkzDirSection from "./settings/WorkzDirSection";
import AppChangesSection from "./settings/AppChangesSection";
import RemoteSection from "./settings/RemoteSection";
import DiagnosticsSection from "./settings/DiagnosticsSection";
import DelegationSection from "./settings/DelegationSection";
import DropdownSelect from "../../components/DropdownSelect";
import "./SettingsPanel.css";

import ModelPricesSection, { pricedModels } from "./settings/ModelPricesSection";

type SettingsTab = "general" | "models" | "agent" | "assistants" | "extensions" | "rules" | "hooks";

interface SettingsPanelProps {
  onClose: () => void;
  projectDir?: string | null;
}

const PROVIDER_KEYS = [
  "anthropic",
  "openai",
  "deepseek",
  "qwen",
  "minimax",
  "zhipu",
  "kimi",
  "custom",
] as const;

const MODEL_PLACEHOLDERS: Record<string, string> = {
  anthropic: "claude-sonnet-4-5",
  openai: "gpt-4o",
  deepseek: "deepseek-chat",
  qwen: "qwen3-max",
  minimax: "MiniMax-M2.5",
  zhipu: "glm-5",
  kimi: "kimi-k2.5",
  custom: "your-model-id",
};

const PROVIDER_BASE_URLS: Record<string, string> = {
  openai: "https://api.openai.com/v1",
  deepseek: "https://api.deepseek.com/v1",
  qwen: "https://dashscope.aliyuncs.com/compatible-mode/v1",
  custom: "https://api.example.com/v1",
};

const BASE_URL_PROVIDERS = ["custom", "qwen", "deepseek", "openai"];

function nextBaseUrl(current: string, prevProvider: string, nextProvider: string): string {
  const prevDefault = PROVIDER_BASE_URLS[prevProvider];
  if (current.trim() && current !== prevDefault) return current;
  if (nextProvider === "custom") return "";
  return PROVIDER_BASE_URLS[nextProvider] ?? "";
}

const KEY_FIELDS: Record<string, keyof LlmSettings> = {
  anthropic: "anthropic_api_key",
  openai: "openai_api_key",
  deepseek: "deepseek_api_key",
  qwen: "qwen_api_key",
  minimax: "minimax_api_key",
  zhipu: "zhipu_api_key",
  kimi: "kimi_api_key",
  custom: "openai_api_key",
};

const DEFAULT_FORM: LlmSettings = {
  provider: "anthropic",
  model: "",
  custom_base_url: "",
  max_tokens: 0,
  context_window: 0,
  max_iterations: 200,
  subagent_max_iterations: 0,
  subagent_timeout_secs: 0,
  policy_mode: "balanced",
  confirm_shell_commands: true,
  confirm_file_writes: true,
  always_allow_tools: [],
  enable_streaming: true,
  temperature: null,
  top_p: null,
  thinking: null,
  language: "zh",
  vision_enabled: false,
  anthropic_api_key: "",
  openai_api_key: "",
  deepseek_api_key: "",
  qwen_api_key: "",
  minimax_api_key: "",
  zhipu_api_key: "",
  kimi_api_key: "",
  llm_providers: [],
  model_prices: [],
  mcp_servers: [],
};

interface SamplingValue {
  temperature?: number | null;
  top_p?: number | null;
  thinking?: boolean | null;
  stream?: boolean | null;
}

function parseOptionalNumber(raw: string, min: number, max: number): number | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const n = Number(trimmed);
  if (!Number.isFinite(n)) return null;
  return Math.min(max, Math.max(min, n));
}

function triState(v: boolean | null | undefined): string {
  return v === true ? "on" : v === false ? "off" : "default";
}

function fromTriState(v: string): boolean | null {
  return v === "on" ? true : v === "off" ? false : null;
}

function SamplingFields({
  value,
  onChange,
  showStream,
  idPrefix,
}: {
  value: SamplingValue;
  onChange: (patch: SamplingValue) => void;
  showStream: boolean;
  idPrefix: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <div className="agentz-settings-field">
        <label htmlFor={`${idPrefix}-temperature`}>{t("settings.temperature")}</label>
        <input
          id={`${idPrefix}-temperature`}
          type="number"
          min={0}
          max={2}
          step={0.1}
          value={value.temperature ?? ""}
          placeholder={t("settings.samplingDefault")}
          onChange={(e) => onChange({ temperature: parseOptionalNumber(e.target.value, 0, 2) })}
        />
        <p className="agentz-settings-hint">{t("settings.temperatureHint")}</p>
      </div>
      <div className="agentz-settings-field">
        <label htmlFor={`${idPrefix}-top-p`}>{t("settings.topP")}</label>
        <input
          id={`${idPrefix}-top-p`}
          type="number"
          min={0}
          max={1}
          step={0.05}
          value={value.top_p ?? ""}
          placeholder={t("settings.samplingDefault")}
          onChange={(e) => onChange({ top_p: parseOptionalNumber(e.target.value, 0, 1) })}
        />
      </div>
      <div className="agentz-settings-field">
        <label>{t("settings.thinking")}</label>
        <DropdownSelect
          variant="field"
          value={value.thinking === true ? "on" : "default"}
          options={[
            { id: "default", label: t("settings.thinkingOff") },
            { id: "on", label: t("settings.thinkingOn") },
          ]}
          onChange={(v) => onChange({ thinking: fromTriState(v) })}
        />
        <p className="agentz-settings-hint">{t("settings.thinkingHint")}</p>
      </div>
      {showStream && (
        <div className="agentz-settings-field">
          <label>{t("settings.llmProviderStream")}</label>
          <DropdownSelect
            variant="field"
            value={triState(value.stream)}
            options={[
              { id: "default", label: t("settings.streamInherit") },
              { id: "on", label: t("settings.streamOn") },
              { id: "off", label: t("settings.streamOff") },
            ]}
            onChange={(v) => onChange({ stream: fromTriState(v) })}
          />
        </div>
      )}
    </>
  );
}

const EMPTY_MCP_SERVER: McpServerConfig = {
  name: "",
  transport: "stdio",
  command: "",
  args: [],
  url: "",
  env: {},
  enabled: true,
};

const EMPTY_LLM_PROVIDER: LlmProviderConfig = {
  id: "",
  label: "",
  provider: "anthropic",
  model: "",
  api_key: "",
  base_url: "",
  max_tokens: 0,
  context_window: 0,
  temperature: null,
  top_p: null,
  thinking: null,
  stream: null,
};

function parseEnv(text: string): Record<string, string> {
  const env: Record<string, string> = Object.create(null);
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const eq = trimmed.indexOf("=");
    if (eq <= 0) continue;
    env[trimmed.slice(0, eq).trim()] = trimmed.slice(eq + 1).trim();
  }
  return env;
}

function McpMapEditor({ value, onChange, placeholder }: {
  value: Record<string, string>;
  onChange: (next: Record<string, string>) => void;
  placeholder: string;
}) {
  const [draft, setDraft] = useState(() => Object.entries(value).map(([key, entry]) => `${key}=${entry}`).join("\n"));
  return <textarea rows={2} value={draft} onChange={(event) => {
    const next = event.target.value;
    setDraft(next);
    onChange(parseEnv(next));
  }} placeholder={placeholder} />;
}

function toForm(data: SettingsResponse): LlmSettings {
  return {
    provider: data.provider || "anthropic",
    model: data.model || "",
    custom_base_url: data.custom_base_url || "",
    max_tokens: data.max_tokens ?? 0,
    max_iterations: data.max_iterations ?? 200,
    subagent_max_iterations: data.subagent_max_iterations ?? 0,
    subagent_timeout_secs: data.subagent_timeout_secs ?? 0,
    context_window: data.context_window ?? 0,
    policy_mode: data.policy_mode || "balanced",
    confirm_shell_commands: data.confirm_shell_commands ?? true,
    confirm_file_writes: data.confirm_file_writes ?? true,
    always_allow_tools: data.always_allow_tools ?? [],
    enable_streaming: data.enable_streaming ?? true,
    temperature: data.temperature ?? null,
    top_p: data.top_p ?? null,
    thinking: data.thinking ?? null,
    language: data.language === "en" ? "en" : "zh",
    vision_enabled: data.vision_enabled ?? false,
    anthropic_api_key: data.anthropic_api_key || "",
    openai_api_key: data.openai_api_key || "",
    deepseek_api_key: data.deepseek_api_key || "",
    qwen_api_key: data.qwen_api_key || "",
    minimax_api_key: data.minimax_api_key || "",
    zhipu_api_key: data.zhipu_api_key || "",
    kimi_api_key: data.kimi_api_key || "",
    llm_providers: data.llm_providers ?? [],
    model_prices: data.model_prices ?? [],
    mcp_servers: data.mcp_servers ?? [],
  };
}

export default function SettingsPanel({ onClose, projectDir = null }: SettingsPanelProps) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<SettingsTab>("general");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [configDir, setConfigDir] = useState("");
  const [configured, setConfigured] = useState(false);
  const [secretPresence, setSecretPresence] = useState<Record<string, boolean>>({});
  const [initialMcpMaps, setInitialMcpMaps] = useState<Record<string, { env: string[]; headers: string[] }>>({});
  const [pendingSecretDeletes, setPendingSecretDeletes] = useState<string[]>([]);
  const [form, setForm] = useState<LlmSettings>(DEFAULT_FORM);
  const [showKey, setShowKey] = useState(false);
  const [llmEditIdx, setLlmEditIdx] = useState<number | null>(null);
  const [llmEditForm, setLlmEditForm] = useState<LlmProviderConfig>(EMPTY_LLM_PROVIDER);
  const [llmEditDeleteKey, setLlmEditDeleteKey] = useState(false);
  const [llmShowKey, setLlmShowKey] = useState(false);
  const effectiveEditWindow = (llmEditForm.context_window ?? 0) > 0 ? (llmEditForm.context_window ?? 0) : form.context_window;
  // Global flash model used by delegated sub-agents.
  const [flashProviderId, setFlashProviderId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const data = await getSettings();
        if (cancelled) return;
        setConfigDir(data.config_dir);
        setConfigured(data.is_configured);
        setSecretPresence(Object.fromEntries(data.secret_references.map(({ reference, exists }) => [reference, exists])));
        setInitialMcpMaps(Object.fromEntries(data.mcp_servers.map((server) => [server.name, {
          env: Object.keys(server.env), headers: Object.keys(server.headers ?? {}),
        }])));
        const next = toForm(data);
        setForm(next);
        if (next.language === "zh" || next.language === "en") {
          setLanguage(next.language);
        }
        try {
          const flash = await getFlashProvider();
          if (!cancelled) setFlashProviderId(flash);
        } catch {
          if (!cancelled) setFlashProviderId(null);
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const update = useCallback(<K extends keyof LlmSettings>(key: K, value: LlmSettings[K]) => {
    setForm((prev) => {
      const next = { ...prev, [key]: value };
      if (key === "language" && (value === "zh" || value === "en")) {
        setLanguage(value);
      }
      return next;
    });
  }, []);

  const updateMcp = useCallback((idx: number, patch: Partial<McpServerConfig>) => {
    setForm((prev) => ({
      ...prev,
      mcp_servers: prev.mcp_servers.map((s, i) => (i === idx ? { ...s, ...patch } : s)),
    }));
  }, []);

  const stageMcpDelete = useCallback((idx: number, field: "command" | "url" | "args") => {
    const server = form.mcp_servers[idx];
    if (!server?.name) return;
    const reference = `mcp:${server.name}:${field}`;
    updateMcp(idx, field === "args" ? { args: [] } : { [field]: "" });
    setPendingSecretDeletes((previous) => previous.includes(reference) ? previous : [...previous, reference]);
  }, [form.mcp_servers, updateMcp]);

  const handleSave = useCallback(async () => {
    setError(null);
    setBusy(true);
    try {
      // Host applies metadata and reference operations to one settings snapshot.
      const { updates, replacements } = splitSettingsSecretUpdates(form);
      const mapDeletions = form.mcp_servers.flatMap((server) => {
        const initial = initialMcpMaps[server.name];
        if (!initial) return [];
        return [
          ...initial.env.filter((key) => !Object.prototype.hasOwnProperty.call(server.env, key)).map((key) => `mcp-env:${JSON.stringify([server.name, key])}`),
          ...initial.headers.filter((key) => !Object.prototype.hasOwnProperty.call(server.headers ?? {}, key)).map((key) => `mcp-header:${JSON.stringify([server.name, key])}`),
        ];
      });
      const deletions = [...pendingSecretDeletes, ...mapDeletions].filter((reference) => {
        const validProvider = !reference.startsWith("provider:") || form.llm_providers.some((p) => `provider:${p.id}` === reference);
        const validMcp = !reference.startsWith("mcp:") || form.mcp_servers.some((server) =>
          (["command", "url", "args"] as const).some((field) => `mcp:${server.name}:${field}` === reference));
        return validProvider && validMcp;
      });
      const result = await saveSettingsWithSecrets(updates, deletions, replacements);
      setPendingSecretDeletes([]);
      setConfigured(result.is_configured);
      setSecretPresence(Object.fromEntries(result.secret_references.map(({ reference, exists }) => [reference, exists])));
      setInitialMcpMaps(Object.fromEntries(result.mcp_servers.map((server) => [server.name, {
        env: Object.keys(server.env), headers: Object.keys(server.headers ?? {}),
      }])));
      setForm(toForm(result));
      if (result.language === "zh" || result.language === "en") {
        setLanguage(result.language);
      }
      // Persist the flash selection, dropping it if the provider was removed.
      const validFlash =
        flashProviderId && form.llm_providers.some((p) => p.id === flashProviderId)
          ? flashProviderId
          : null;
      try {
        await setFlashProvider(validFlash);
      } catch {
        /* non-fatal: flash selection is best-effort */
      }
      notifySettingsRefresh();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [form, onClose, flashProviderId, pendingSecretDeletes, initialMcpMaps]);

  const provider = form.provider || "anthropic";
  const apiKeyField = KEY_FIELDS[provider] ?? "anthropic_api_key";
  const apiKeyValue = form[apiKeyField] as string;
  const apiKeyReference = `model:${String(apiKeyField).replace(/_api_key$/, "")}`;

  const providerLabel = (p: string) => {
    const map: Record<string, string> = {
      anthropic: t("settings.providerAnthropic"),
      openai: t("settings.providerOpenai"),
      deepseek: t("settings.providerDeepseek"),
      qwen: t("settings.providerQwen"),
      minimax: t("settings.providerMinimax"),
      zhipu: t("settings.providerZhipu"),
      kimi: t("settings.providerKimi"),
      custom: t("settings.providerCustom"),
    };
    return map[p] || p;
  };

  const panel = (
    <div className="agentz-settings-overlay" onClick={onClose}>
      <div className="agentz-settings-panel" onClick={(e) => e.stopPropagation()}>
        <div className="agentz-settings-header">
          <div>
            <span>{t("settings.titleHub")}</span>
            <p className="agentz-settings-subtitle">{t("settings.subtitleHub")}</p>
          </div>
          <button type="button" onClick={onClose} title={t("common.close")}>
            ✕
          </button>
        </div>

        <div className="agentz-settings-tabs" role="tablist">
          {(
            [
              ["general", t("settings.tabGeneral")],
              ["models", t("settings.tabModels")],
              ["agent", t("settings.tabAgent")],
              ["assistants", t("settings.tabChannels")],
              ["extensions", t("settings.tabExtensions")],
              ["rules", t("settings.tabRules")],
              ["hooks", t("settings.tabHooks")],
            ] as [SettingsTab, string][]
          ).map(([key, label]) => (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={tab === key}
              className={`agentz-settings-tab ${tab === key ? "active" : ""}`}
              onClick={() => setTab(key)}
            >
              {label}
            </button>
          ))}
        </div>

        {tab === "assistants" && (
          <div className="agentz-settings-body">
            <AssistantsTab />
          </div>
        )}
        {tab === "extensions" && (
          <div className="agentz-settings-body">
            <ExtensionsManager projectDir={projectDir} />
            {projectDir && <RemoteSection projectDir={projectDir} />}
          </div>
        )}
        {tab === "rules" && (
          <div className="agentz-settings-body">
            <RulesTab projectDir={projectDir} />
          </div>
        )}
        {tab === "hooks" && (
          <div className="agentz-settings-body">
            <HooksTab projectDir={projectDir} />
          </div>
        )}

        {tab === "general" && (
          <>
            {error && <div className="agentz-settings-error">{error}</div>}
            {loading ? (
              <div className="agentz-settings-loading">{t("settings.loading")}</div>
            ) : (
              <>
                <div className="agentz-settings-body">
                  <section className="agentz-settings-section">
                    <div className="agentz-settings-field">
                      <label htmlFor="agentz-settings-language">{t("settings.language")}</label>
                      <DropdownSelect
                        id="agentz-settings-language"
                        variant="field"
                        value={form.language}
                        options={[
                          { id: "zh", label: t("settings.languageZh") },
                          { id: "en", label: t("settings.languageEn") },
                        ]}
                        onChange={(v) => update("language", v)}
                      />
                    </div>
                  </section>

                  <WorkzDirSection />
                  <AppChangesSection />

                  <div className="agentz-settings-status">
                    <span
                      className={`agentz-settings-status-dot ${configured ? "ok" : "missing"}`}
                    />
                    <span style={{ color: configured ? "var(--success)" : "var(--error)" }}>
                      {configured ? t("settings.configuredOk") : t("settings.configuredMissing")}
                    </span>
                  </div>

                  <div className="agentz-settings-config-path">
                    {t("settings.configDir")}：<code>{configDir || "—"}</code>
                    <br />
                    {t("settings.configDirHint")}
                  </div>

                  <DiagnosticsSection />
                </div>

                <div className="agentz-settings-footer">
                  <button type="button" className="agentz-settings-cancel" onClick={onClose}>
                    {t("common.cancel")}
                  </button>
                  <button
                    type="button"
                    className="agentz-settings-save"
                    onClick={() => void handleSave()}
                    disabled={busy}
                  >
                    {busy ? t("common.saving") : t("common.save")}
                  </button>
                </div>
              </>
            )}
          </>
        )}

        {(tab === "models" || tab === "agent") && (
          <>
            {error && <div className="agentz-settings-error">{error}</div>}

            {loading ? (
              <div className="agentz-settings-loading">{t("settings.loading")}</div>
            ) : (
          <>
            <div className="agentz-settings-body">
              {tab === "models" && (
              <>
              <section className="agentz-settings-section">
                <h3>{t("settings.aiProvider")}</h3>

                <div className="agentz-settings-field">
                  <label htmlFor="agentz-settings-provider">{t("settings.provider")}</label>
                  <DropdownSelect
                    id="agentz-settings-provider"
                    variant="field"
                    value={provider}
                    options={PROVIDER_KEYS.map((p) => ({ id: p, label: providerLabel(p) }))}
                    onChange={(v) => {
                      update("custom_base_url", nextBaseUrl(form.custom_base_url, provider, v));
                      update("provider", v);
                    }}
                  />
                </div>

                <div className="agentz-settings-field">
                  <label htmlFor="agentz-settings-model">{t("settings.model")}</label>
                  <input
                    id="agentz-settings-model"
                    value={form.model}
                    onChange={(e) => update("model", e.target.value)}
                    placeholder={MODEL_PLACEHOLDERS[provider] ?? "model-id"}
                  />
                </div>

                {BASE_URL_PROVIDERS.includes(provider) && (
                  <div className="agentz-settings-field">
                    <label htmlFor="agentz-settings-base-url">{t("settings.baseUrl")}</label>
                    <input
                      id="agentz-settings-base-url"
                      value={form.custom_base_url}
                      onChange={(e) => update("custom_base_url", e.target.value)}
                      placeholder={PROVIDER_BASE_URLS[provider] ?? ""}
                    />
                  </div>
                )}

                <div className="agentz-settings-field">
                  <label htmlFor="agentz-settings-api-key">{t("settings.apiKey")}</label>
                  <div className="agentz-settings-key-row">
                    <input
                      id="agentz-settings-api-key"
                      type={showKey ? "text" : "password"}
                      value={apiKeyValue}
                      onChange={(e) => update(apiKeyField, e.target.value)}
                      placeholder={
                        secretPresence[apiKeyReference] && !pendingSecretDeletes.includes(apiKeyReference) ? t("settings.apiKeyKeep") : t("settings.apiKeyNew")
                      }
                    />
                    <button
                      type="button"
                      className="agentz-settings-key-toggle"
                      onClick={() => setShowKey((v) => !v)}
                    >
                      {showKey ? t("common.hide") : t("common.show")}
                    </button>
                    {secretPresence[apiKeyReference] && !pendingSecretDeletes.includes(apiKeyReference) && (
                      <button type="button" className="agentz-settings-key-toggle" onClick={() => {
                        update(apiKeyField, "");
                        setPendingSecretDeletes((previous) => [...previous, apiKeyReference]);
                      }}>{t("settings.deleteApiKey")}</button>
                    )}
                  </div>
                  <p className="agentz-settings-hint">{t("settings.apiKeyHint")}</p>
                </div>

                <div className="agentz-settings-field">
                  <label htmlFor="agentz-settings-max-tokens">{t("settings.maxTokens")}</label>
                  <input
                    id="agentz-settings-max-tokens"
                    type="number"
                    min={0}
                    value={form.max_tokens}
                    onChange={(e) => update("max_tokens", Number(e.target.value) || 0)}
                  />
                  <p className="agentz-settings-hint">{t("settings.maxTokensHint")}</p>
                </div>

                <div className="agentz-settings-field">
                  <label htmlFor="agentz-settings-context">{t("settings.contextWindow")}</label>
                  <input
                    id="agentz-settings-context"
                    type="number"
                    min={0}
                    step={1024}
                    value={form.context_window}
                    onChange={(e) => update("context_window", Number(e.target.value) || 0)}
                  />
                  <p className="agentz-settings-hint">{t("settings.contextWindowHint")}</p>
                </div>

                <div className="agentz-settings-field">
                  <label className="agentz-settings-checkbox">
                    <input
                      type="checkbox"
                      checked={form.vision_enabled}
                      onChange={(e) => update("vision_enabled", e.target.checked)}
                    />
                    {t("settings.visionEnabled")}
                  </label>
                  <p className="agentz-settings-hint">{t("settings.visionEnabledHint")}</p>
                </div>

                <div className="agentz-settings-field">
                  <label className="agentz-settings-checkbox">
                    <input
                      type="checkbox"
                      checked={form.enable_streaming}
                      onChange={(e) => update("enable_streaming", e.target.checked)}
                    />
                    {t("settings.enableStreaming")}
                  </label>
                </div>

                <SamplingFields
                  idPrefix="agentz-settings-main"
                  value={form}
                  showStream={false}
                  onChange={(patch) => setForm((f) => ({ ...f, ...patch }))}
                />
              </section>

              <section className="agentz-settings-section">
                <h3>{t("settings.llmProviders")}</h3>
                <p className="agentz-settings-hint">{t("settings.llmProvidersHint")}</p>

                {form.llm_providers.length > 0 && (
                  <div className="agentz-llm-provider-list">
                    {form.llm_providers.map((p, idx) => (
                      <div key={p.id} className="agentz-llm-provider-row">
                        <div className="agentz-llm-provider-info">
                          <strong>{p.label || p.id}</strong>
                          <span className="agentz-llm-provider-meta">
                            {p.provider} · {p.model || "—"}
                          </span>
                          <label
                            className="agentz-llm-provider-flash"
                            title={t("settings.flashModelHint")}
                          >
                            <input
                              type="checkbox"
                              checked={flashProviderId === p.id}
                              onChange={(e) =>
                                setFlashProviderId(e.target.checked ? p.id : null)
                              }
                            />
                            {t("settings.flashModel")}
                          </label>
                        </div>
                        <div className="agentz-llm-provider-actions">
                          <button
                            type="button"
                            onClick={() => {
                              setLlmEditIdx(idx);
                              setLlmEditForm({ ...p, api_key: "" });
                              setLlmEditDeleteKey(false);
                              setLlmShowKey(false);
                            }}
                          >
                            {t("common.edit")}
                          </button>
                          <button
                            type="button"
                            className="danger"
                            onClick={() =>
                              update(
                                "llm_providers",
                                form.llm_providers.filter((_, i) => i !== idx),
                              )
                            }
                          >
                            {t("chat.delete")}
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}

                {llmEditIdx !== null ? (
                  <div className="agentz-llm-provider-form">
                    <h4>{llmEditIdx === -1 ? t("settings.llmProviderAdd") : t("settings.llmProviderEdit")}</h4>
                    <div className="agentz-llm-provider-grid">
                      <div className="agentz-settings-field">
                        <label>{t("settings.llmProviderId")}</label>
                        <input
                          value={llmEditForm.id}
                          disabled={llmEditIdx !== -1}
                          onChange={(e) => setLlmEditForm((f) => ({ ...f, id: e.target.value }))}
                          placeholder="my-claude"
                        />
                      </div>
                      <div className="agentz-settings-field">
                        <label>{t("settings.llmProviderLabel")}</label>
                        <input
                          value={llmEditForm.label}
                          onChange={(e) => setLlmEditForm((f) => ({ ...f, label: e.target.value }))}
                          placeholder="Claude Sonnet"
                        />
                      </div>
                      <div className="agentz-settings-field">
                        <label>{t("settings.provider")}</label>
                        <DropdownSelect
                          variant="field"
                          value={llmEditForm.provider}
                          options={PROVIDER_KEYS.map((p) => ({ id: p, label: providerLabel(p) }))}
                          onChange={(v) =>
                            setLlmEditForm((f) => ({
                              ...f,
                              provider: v,
                              base_url: nextBaseUrl(f.base_url, f.provider, v),
                            }))
                          }
                        />
                      </div>
                      <div className="agentz-settings-field">
                        <label>{t("settings.model")}</label>
                        <input
                          value={llmEditForm.model}
                          onChange={(e) => setLlmEditForm((f) => ({ ...f, model: e.target.value }))}
                          placeholder={MODEL_PLACEHOLDERS[llmEditForm.provider] ?? "model-id"}
                        />
                      </div>
                      <div className="agentz-settings-field agentz-llm-span-2">
                        <label>{t("settings.apiKey")}</label>
                        <div className="agentz-settings-key-row">
                          <input
                            type={llmShowKey ? "text" : "password"}
                            value={llmEditForm.api_key}
                            onChange={(e) => setLlmEditForm((f) => ({ ...f, api_key: e.target.value }))}
                            placeholder={llmEditIdx !== -1 && secretPresence[`provider:${llmEditForm.id}`] && !llmEditDeleteKey ? t("settings.apiKeyKeep") : t("settings.apiKeyNew")}
                          />
                          <button type="button" className="agentz-settings-key-toggle" onClick={() => setLlmShowKey((v) => !v)}>
                            {llmShowKey ? t("common.hide") : t("common.show")}
                          </button>
                          {llmEditIdx !== -1 && secretPresence[`provider:${llmEditForm.id}`] && !llmEditDeleteKey && (
                            <button type="button" className="agentz-settings-key-toggle" onClick={() => {
                              setLlmEditForm((previous) => ({ ...previous, api_key: "" }));
                              setLlmEditDeleteKey(true);
                            }}>{t("settings.deleteApiKey")}</button>
                          )}
                        </div>
                      </div>
                      {BASE_URL_PROVIDERS.includes(llmEditForm.provider) && (
                        <div className="agentz-settings-field agentz-llm-span-2">
                          <label>{t("settings.baseUrl")}</label>
                          <input
                            value={llmEditForm.base_url}
                            placeholder={PROVIDER_BASE_URLS[llmEditForm.provider] ?? ""}
                            onChange={(e) => setLlmEditForm((f) => ({ ...f, base_url: e.target.value }))}
                          />
                        </div>
                      )}
                      <div className="agentz-settings-field">
                        <label>{t("settings.llmProviderMaxTokens")}</label>
                        <input
                          type="number"
                          min={0}
                          value={llmEditForm.max_tokens}
                          onChange={(e) =>
                            setLlmEditForm((f) => ({ ...f, max_tokens: Number(e.target.value) || 0 }))
                          }
                        />
                        <p className="agentz-settings-hint">{t("settings.llmProviderMaxTokensHint")}</p>
                        {effectiveEditWindow > 0 && llmEditForm.max_tokens > effectiveEditWindow / 4 && (
                          <p className="agentz-settings-hint agentz-settings-hint--warn">
                            {t("settings.llmProviderMaxTokensTooLarge", {
                              cap: String(Math.floor(effectiveEditWindow / 4)),
                            })}
                          </p>
                        )}
                      </div>
                      <div className="agentz-settings-field">
                        <label>{t("settings.llmProviderContextWindow")}</label>
                        <input
                          type="number"
                          min={0}
                          step={1024}
                          value={llmEditForm.context_window ?? 0}
                          onChange={(e) =>
                            setLlmEditForm((f) => ({ ...f, context_window: Number(e.target.value) || 0 }))
                          }
                        />
                        <p className="agentz-settings-hint">
                          {t("settings.llmProviderContextWindowHint", {
                            window: form.context_window > 0 ? String(form.context_window) : t("settings.contextWindowAuto"),
                          })}
                        </p>
                      </div>
                      <SamplingFields
                        idPrefix="agentz-llm-edit"
                        value={llmEditForm}
                        showStream
                        onChange={(patch) => setLlmEditForm((f) => ({ ...f, ...patch }))}
                      />
                    </div>
                    <div className="agentz-llm-provider-form-actions">
                      <button
                        type="button"
                        className="agentz-settings-save"
                        onClick={() => {
                          const id = llmEditForm.id.trim();
                          const model = llmEditForm.model.trim();
                          if (!id || !model) return;
                          if (llmEditDeleteKey && llmEditIdx !== -1 && form.llm_providers[llmEditIdx]?.id === id) {
                            setPendingSecretDeletes((previous) => [...previous, `provider:${id}`]);
                          }
                          if (llmEditIdx === -1) {
                            if (form.llm_providers.some((p) => p.id === id)) return;
                            update("llm_providers", [...form.llm_providers, { ...llmEditForm, id, model }]);
                          } else {
                            update(
                              "llm_providers",
                              form.llm_providers.map((p, i) =>
                                i === llmEditIdx ? { ...llmEditForm, id, model } : p,
                              ),
                            );
                          }
                          setLlmEditIdx(null);
                          setLlmEditDeleteKey(false);
                        }}
                      >
                        {t("common.save")}
                      </button>
                      <button type="button" className="agentz-settings-cancel" onClick={() => { setLlmEditIdx(null); setLlmEditDeleteKey(false); }}>
                        {t("common.cancel")}
                      </button>
                    </div>
                  </div>
                ) : (
                  <button
                    type="button"
                    className="agentz-llm-add"
                    onClick={() => {
                      setLlmEditIdx(-1);
                      setLlmEditDeleteKey(false);
                      setLlmEditForm(EMPTY_LLM_PROVIDER);
                      setLlmShowKey(false);
                    }}
                  >
                    + {t("settings.llmProviderAdd")}
                  </button>
                )}
              </section>

              <ModelPricesSection
                models={pricedModels(form, form.llm_providers)}
                prices={form.model_prices ?? []}
                onChange={(prices) => update("model_prices", prices)}
              />
              </>
              )}

              {tab === "agent" && (
              <>
              <section className="agentz-settings-section">
                <h3>{t("settings.agentRuntime")}</h3>
                <div className="agentz-settings-field">
                    <label htmlFor="agentz-settings-policy">{t("settings.policyMode")}</label>
                    <DropdownSelect
                      id="agentz-settings-policy"
                      variant="field"
                      value={form.policy_mode}
                      options={[
                        { id: "balanced", label: t("settings.policyBalanced") },
                        { id: "strict", label: t("settings.policyStrict") },
                        { id: "dev", label: t("settings.policyDev") },
                      ]}
                      onChange={(v) => update("policy_mode", v)}
                    />
                    <p className="agentz-settings-hint">{t("settings.policyModeHint")}</p>
                  </div>
                <div className="agentz-settings-field">
                  <label htmlFor="agentz-settings-max-iterations">{t("settings.maxIterations")}</label>
                  <input
                    id="agentz-settings-max-iterations"
                    type="number"
                    min={1}
                    max={1000}
                    value={form.max_iterations}
                    onChange={(e) => update("max_iterations", Math.min(1000, Math.max(1, Number(e.target.value) || 1)))}
                  />
                  <p className="agentz-settings-hint">{t("settings.maxIterationsHint")}</p>
                </div>

                <div className="agentz-settings-field">
                  <label htmlFor="agentz-settings-subagent-iterations">{t("settings.subagentMaxIterations")}</label>
                  <input
                    id="agentz-settings-subagent-iterations"
                    type="number"
                    min={0}
                    max={1000}
                    value={form.subagent_max_iterations ?? 0}
                    onChange={(e) => update("subagent_max_iterations", Math.min(1000, Math.max(0, Number(e.target.value) || 0)))}
                  />
                  <p className="agentz-settings-hint">
                    {t("settings.subagentMaxIterationsHint", { value: String(Math.max(10, Math.floor(form.max_iterations / 2))) })}
                  </p>
                </div>

                <div className="agentz-settings-field">
                  <label htmlFor="agentz-settings-subagent-timeout">{t("settings.subagentTimeout")}</label>
                  <input
                    id="agentz-settings-subagent-timeout"
                    type="number"
                    min={0}
                    max={3600}
                    value={form.subagent_timeout_secs ?? 0}
                    onChange={(e) => update("subagent_timeout_secs", Math.min(3600, Math.max(0, Number(e.target.value) || 0)))}
                  />
                  <p className="agentz-settings-hint">{t("settings.subagentTimeoutHint")}</p>
                </div>

                <div className="agentz-settings-field">
                  <label className="agentz-settings-checkbox">
                    <input
                      type="checkbox"
                      checked={form.confirm_shell_commands ?? true}
                      onChange={(e) => {
                        const checked = e.target.checked;
                        setForm((prev) => {
                          const current = prev.always_allow_tools ?? [];
                          const nextTools = checked
                            ? current.filter((item) => item !== "shell" && item !== "code_run")
                            : current.includes("shell")
                              ? current
                              : [...current, "shell"];
                          return {
                            ...prev,
                            confirm_shell_commands: checked,
                            always_allow_tools: nextTools,
                          };
                        });
                      }}
                    />
                    {t("settings.confirmShellCommands")}
                  </label>
                  <p className="agentz-settings-hint">{t("settings.confirmShellCommandsHint")}</p>
                </div>

                <div className="agentz-settings-field">
                  <label className="agentz-settings-checkbox">
                    <input
                      type="checkbox"
                      checked={form.confirm_file_writes ?? true}
                      onChange={(e) => update("confirm_file_writes", e.target.checked)}
                    />
                    {t("settings.confirmFileWrites")}
                  </label>
                  <p className="agentz-settings-hint">{t("settings.confirmFileWritesHint")}</p>
                </div>

                <div className="agentz-settings-field">
                  <label>{t("settings.alwaysAllowTools")}</label>
                  <div className="agentz-permission-chips">
                    {[
                      { id: "shell", label: t("settings.toolPermShell") },
                      { id: "code_run", label: t("settings.toolPermCodeRun") },
                      { id: "read_lints", label: t("settings.toolPermReadLints") },
                      { id: "lsp", label: t("settings.toolPermLsp") },
                      { id: "browser", label: t("settings.toolPermBrowser") },
                      { id: "*", label: t("settings.toolPermAll") },
                    ].map((tool) => {
                      const list = form.always_allow_tools ?? [];
                      const active =
                        list.includes(tool.id) ||
                        (tool.id === "shell" && form.confirm_shell_commands === false);
                      return (
                        <button
                          key={tool.id}
                          type="button"
                          className={`agentz-permission-chip ${active ? "active" : ""}`}
                          onClick={() => {
                            setForm((prev) => {
                              const current = prev.always_allow_tools ?? [];
                              const isCurrentlyActive =
                                current.includes(tool.id) ||
                                (tool.id === "shell" && prev.confirm_shell_commands === false);
                              const nextTools = isCurrentlyActive
                                ? current.filter((item) => item !== tool.id)
                                : [...current.filter((item) => item !== tool.id), tool.id];
                              const nextConfirmShell =
                                tool.id === "shell"
                                  ? isCurrentlyActive
                                  : prev.confirm_shell_commands;
                              return {
                                ...prev,
                                confirm_shell_commands: nextConfirmShell,
                                always_allow_tools: nextTools,
                              };
                            });
                          }}
                        >
                          {active ? "✓ " : ""}
                          {tool.label}
                        </button>
                      );
                    })}
                  </div>
                  <p className="agentz-settings-hint">{t("settings.alwaysAllowToolsHint")}</p>
                </div>

              </section>

              <DelegationSection />

              <section className="agentz-settings-section">
                <h3>{t("settings.mcpServers")}</h3>
                <p className="agentz-settings-hint">{t("settings.mcpServersHint")}</p>

                {form.mcp_servers.map((srv, idx) => (
                  <div key={idx} className="agentz-mcp-server">
                    <div className="agentz-mcp-server-head">
                      <input
                        className="agentz-mcp-name"
                        value={srv.name}
                        onChange={(e) => updateMcp(idx, { name: e.target.value })}
                        placeholder={t("settings.mcpName")}
                      />
                      <DropdownSelect
                        variant="inline"
                        value={srv.transport}
                        options={[
                          { id: "stdio", label: "stdio" },
                          { id: "sse", label: "sse" },
                        ]}
                        onChange={(v) => updateMcp(idx, { transport: v })}
                      />
                      <label className="agentz-settings-checkbox">
                        <input
                          type="checkbox"
                          checked={srv.enabled}
                          onChange={(e) => updateMcp(idx, { enabled: e.target.checked })}
                        />
                        {t("settings.mcpEnabled")}
                      </label>
                      <button
                        type="button"
                        className="danger"
                        onClick={() =>
                          update(
                            "mcp_servers",
                            form.mcp_servers.filter((_, i) => i !== idx),
                          )
                        }
                      >
                        {t("chat.delete")}
                      </button>
                    </div>
                    {srv.transport === "sse" ? (
                      <div className="agentz-settings-field">
                        <label>{t("settings.mcpUrl")}</label>
                        <div className="agentz-settings-key-row">
                        <input
                          value={srv.url}
                          onChange={(e) => updateMcp(idx, { url: e.target.value })}
                          placeholder={srv.url_configured && !pendingSecretDeletes.includes(`mcp:${srv.name}:url`) ? t("settings.mcpHiddenValue") : "http://localhost:3000"}
                        />
                        {srv.url_configured && !pendingSecretDeletes.includes(`mcp:${srv.name}:url`) && (
                          <button type="button" className="agentz-settings-key-toggle" onClick={() => stageMcpDelete(idx, "url")}>{t("settings.deleteMcpValue")}</button>
                        )}
                        </div>
                      </div>
                    ) : (
                      <>
                        <div className="agentz-settings-field">
                          <label>{t("settings.mcpCommand")}</label>
                          <div className="agentz-settings-key-row">
                          <input
                            value={srv.command}
                            onChange={(e) => updateMcp(idx, { command: e.target.value })}
                            placeholder={srv.command_configured && !pendingSecretDeletes.includes(`mcp:${srv.name}:command`) ? t("settings.mcpHiddenValue") : "npx"}
                          />
                          {srv.command_configured && !pendingSecretDeletes.includes(`mcp:${srv.name}:command`) && (
                            <button type="button" className="agentz-settings-key-toggle" onClick={() => stageMcpDelete(idx, "command")}>{t("settings.deleteMcpValue")}</button>
                          )}
                          </div>
                        </div>
                        <div className="agentz-settings-field">
                          <label>{t("settings.mcpArgs")}</label>
                          <div className="agentz-settings-key-row">
                          <input
                            value={srv.args.join(" ")}
                            onChange={(e) =>
                              updateMcp(idx, {
                                args: e.target.value.split(/\s+/).filter(Boolean),
                              })
                            }
                            placeholder={srv.args_configured && !pendingSecretDeletes.includes(`mcp:${srv.name}:args`) ? t("settings.mcpHiddenValue") : "-y @modelcontextprotocol/server-filesystem /path"}
                          />
                          {srv.args_configured && !pendingSecretDeletes.includes(`mcp:${srv.name}:args`) && (
                            <button type="button" className="agentz-settings-key-toggle" onClick={() => stageMcpDelete(idx, "args")}>{t("settings.deleteMcpValue")}</button>
                          )}
                          </div>
                        </div>
                      </>
                    )}
                    <div className="agentz-settings-field">
                      <label>{t("settings.mcpEnv")}</label>
                      <McpMapEditor value={srv.env} onChange={(env) => updateMcp(idx, { env })}
                        placeholder={"KEY=value\nOTHER=value"} />
                    </div>
                    <div className="agentz-settings-field">
                      <label>{t("settings.mcpHeaders")}</label>
                      <McpMapEditor value={srv.headers ?? {}} onChange={(headers) => updateMcp(idx, { headers })}
                        placeholder={"Authorization=Bearer …"} />
                    </div>
                  </div>
                ))}

                <button
                  type="button"
                  className="agentz-llm-add"
                  onClick={() => update("mcp_servers", [...form.mcp_servers, { ...EMPTY_MCP_SERVER }])}
                >
                  + {t("settings.mcpServerAdd")}
                </button>
              </section>
              </>
              )}
            </div>

            <div className="agentz-settings-footer">
              <button type="button" className="agentz-settings-cancel" onClick={onClose}>
                {t("common.cancel")}
              </button>
              <button
                type="button"
                className="agentz-settings-save"
                onClick={() => void handleSave()}
                disabled={busy}
              >
                {busy ? t("common.saving") : t("common.save")}
              </button>
            </div>
          </>
            )}
          </>
        )}
      </div>
    </div>
  );

  return createPortal(panel, document.body);
}
