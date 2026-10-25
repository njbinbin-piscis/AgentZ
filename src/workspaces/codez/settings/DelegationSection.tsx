import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getDelegation,
  getDelegationMetrics,
  setDelegation,
  type DelegationConfig,
  type DelegationMetrics,
  type DelegationMode,
} from "../../../services/tauri/delegation";

const MODES: Array<{ id: DelegationMode; labelKey: string }> = [
  { id: "auto", labelKey: "settings.delegationAuto" },
  { id: "research", labelKey: "settings.delegationResearch" },
  { id: "off", labelKey: "settings.delegationOff" },
];

export default function DelegationSection() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<DelegationConfig | null>(null);
  const [metrics, setMetrics] = useState<DelegationMetrics | null>(null);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    try {
      const [c, m] = await Promise.all([getDelegation(), getDelegationMetrics()]);
      setConfig(c);
      setMetrics(m);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function save(next: DelegationConfig) {
    setConfig(next);
    setNotice("");
    setError("");
    try {
      setConfig(await setDelegation(next));
      setNotice(t("settings.delegationSaved"));
    } catch (e) {
      setError(String(e));
    }
  }

  if (!config) {
    return error ? (
      <p className="agentz-diagnostics-error" role="alert">
        {error}
      </p>
    ) : null;
  }

  return (
    <section className="agentz-settings-section" aria-label={t("settings.delegationTitle")}>
      <h3>{t("settings.delegationTitle")}</h3>
      <p className="agentz-settings-hint">{t("settings.delegationDescription")}</p>
      <div className="agentz-settings-field">
        <label htmlFor="agentz-delegation-mode">{t("settings.delegationMode")}</label>
        <select
          id="agentz-delegation-mode"
          data-testid="delegation-mode"
          value={config.mode}
          onChange={(e) => void save({ ...config, mode: e.target.value as DelegationMode })}
        >
          {MODES.map((m) => (
            <option key={m.id} value={m.id}>
              {t(m.labelKey)}
            </option>
          ))}
        </select>
      </div>
      <div className="agentz-settings-field">
        <label htmlFor="agentz-delegation-max">{t("settings.delegationMaxConcurrent")}</label>
        <input
          id="agentz-delegation-max"
          data-testid="delegation-max"
          type="number"
          min={1}
          max={16}
          value={config.max_concurrent}
          onChange={(e) =>
            void save({
              ...config,
              max_concurrent: Math.min(16, Math.max(1, Number(e.target.value) || 1)),
            })
          }
        />
      </div>
      {metrics && (
        <div className="agentz-diagnostics-card" data-testid="delegation-metrics">
          <p>{t("settings.delegationMetricsTitle")}</p>
          <p>
            {t("settings.delegationMetricsLine", {
              calls: metrics.calls,
              failures: metrics.failures,
              tools: metrics.sub_tool_calls,
              input: metrics.sub_input_tokens,
              output: metrics.sub_output_tokens,
              fallbacks: metrics.model_fallbacks,
            })}
          </p>
          <button type="button" className="agentz-settings-cancel" onClick={() => void refresh()}>
            {t("settings.delegationRefresh")}
          </button>
        </div>
      )}
      {notice && (
        <p className="agentz-settings-hint" data-testid="delegation-saved">
          {notice}
        </p>
      )}
      {error && (
        <p className="agentz-diagnostics-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
