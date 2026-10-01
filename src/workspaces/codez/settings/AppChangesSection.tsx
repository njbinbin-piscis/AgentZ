import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { notifySettingsRefresh } from "../../../services/settingsRefresh";

interface ChangeEntry {
  id: string;
  ts: number;
  session: string;
  kind: string;
  target: string;
  summary: string;
  diff: { path: string; old: unknown; new: unknown }[];
  rolled_back: boolean;
}

const show = (v: unknown) => (typeof v === "string" ? v : JSON.stringify(v));

export default function AppChangesSection() {
  const { t } = useTranslation();
  const [items, setItems] = useState<ChangeEntry[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    invoke<ChangeEntry[]>("app_changes_list", { limit: 50 })
      .then(setItems)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    load();
    const un = listen("agentz:app-control-updated", load);
    return () => {
      void un.then((f) => f());
    };
  }, [load]);

  const rollback = useCallback(
    async (id: string) => {
      if (!window.confirm(t("appChanges.confirmRollback"))) return;
      try {
        await invoke("app_changes_rollback", { id });
        setError(null);
        notifySettingsRefresh();
        load();
      } catch (e) {
        setError(String(e));
      }
    },
    [load, t],
  );

  return (
    <section className="agentz-settings-section">
      <h3>{t("appChanges.title")}</h3>
      <p className="agentz-settings-hint">{t("appChanges.hint")}</p>
      {items.length === 0 && <p className="agentz-settings-hint">{t("appChanges.empty")}</p>}
      <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
        {items.map((c) => (
          <li key={c.id} style={{ padding: "6px 0", opacity: c.rolled_back ? 0.55 : 1 }}>
            <div style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
              <span>
                <strong>{c.summary}</strong>
                <span style={{ opacity: 0.6, marginLeft: 8 }}>
                  {new Date(c.ts * 1000).toLocaleString()}
                </span>
              </span>
              {c.rolled_back ? (
                <span>{t("appChanges.rolledBack")}</span>
              ) : (
                <button type="button" onClick={() => void rollback(c.id)}>
                  {t("appChanges.rollback")}
                </button>
              )}
            </div>
            {c.diff.slice(0, 6).map((d) => (
              <div key={d.path} style={{ fontSize: 12, opacity: 0.8 }}>
                {d.path}: {show(d.old)} → {show(d.new)}
              </div>
            ))}
            {c.diff.length > 6 && (
              <div style={{ fontSize: 12, opacity: 0.6 }}>… +{c.diff.length - 6}</div>
            )}
          </li>
        ))}
      </ul>
      {error && <div className="agentz-settings-error">{error}</div>}
    </section>
  );
}
