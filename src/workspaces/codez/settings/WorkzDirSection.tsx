import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { openFolderDialog } from "../../../services/tauri";
import {
  workzOverview,
  workzProjectRemove,
  workzSetDefaultDir,
  type WorkzOverview,
} from "../../../services/tauri/workz";

export default function WorkzDirSection() {
  const { t } = useTranslation();
  const [overview, setOverview] = useState<WorkzOverview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    workzOverview()
      .then(setOverview)
      .catch((e) => setError(String(e)));
  }, []);

  const run = useCallback(async (p: Promise<WorkzOverview>) => {
    try {
      setOverview(await p);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const pick = useCallback(async () => {
    const dir = await openFolderDialog(overview?.default_work_dir ?? null);
    if (dir) await run(workzSetDefaultDir(dir));
  }, [overview, run]);

  return (
    <section className="agentz-settings-section">
      <h3>{t("workz.defaultDirTitle")}</h3>
      <p className="agentz-settings-hint">{t("workz.defaultDirHint")}</p>
      <div className="agentz-settings-field">
        <input
          type="text"
          readOnly
          value={overview?.default_work_dir ?? ""}
          style={{ width: "100%" }}
        />
        <div style={{ display: "flex", gap: 8, marginTop: 6 }}>
          <button type="button" onClick={() => void pick()}>
            {t("workz.defaultDirPick")}
          </button>
          <button
            type="button"
            disabled={!overview?.default_is_custom}
            onClick={() => void run(workzSetDefaultDir(null))}
          >
            {t("workz.defaultDirReset")}
          </button>
        </div>
      </div>
      <h4 style={{ margin: "14px 0 6px" }}>{t("workz.projectList")}</h4>
      {overview?.projects.length === 0 && (
        <p className="agentz-settings-hint">{t("workz.noProjects")}</p>
      )}
      <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
        {overview?.projects.map((p) => (
          <li
            key={p.path}
            style={{ display: "flex", justifyContent: "space-between", gap: 8, padding: "3px 0" }}
          >
            <span title={p.path} style={{ opacity: p.exists ? 1 : 0.5 }}>
              {p.kind === "repo" ? "⎇ " : "▤ "}
              {p.name}
              <span style={{ opacity: 0.55, marginLeft: 8 }}>{p.path}</span>
            </span>
            <button type="button" onClick={() => void run(workzProjectRemove(p.path))}>
              {t("workz.removeProject")}
            </button>
          </li>
        ))}
      </ul>
      {error && <div className="agentz-settings-error">{error}</div>}
    </section>
  );
}
