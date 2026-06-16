import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  applyProjectTemplate,
  listProjectTemplates,
  type ProjectTemplateInfo,
} from "../services/tauri/projectTemplates";
import "./ProjectTemplateDialog.css";

function pickLocalized(zh: string, en: string, lang: string): string {
  if (lang.startsWith("zh") && zh.trim()) return zh;
  return en.trim() ? en : zh;
}

interface ProjectTemplateDialogProps {
  projectDir: string;
  onDone: () => void;
  onSkip: () => void;
}

export default function ProjectTemplateDialog({
  projectDir,
  onDone,
  onSkip,
}: ProjectTemplateDialogProps) {
  const { t, i18n } = useTranslation();
  const [templates, setTemplates] = useState<ProjectTemplateInfo[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listProjectTemplates()
      .then((list) => {
        setTemplates(list);
        if (list[0]) setSelected(list[0].id);
      })
      .catch(() => setTemplates([]));
  }, []);

  const selectedTpl = useMemo(
    () => templates.find((tpl) => tpl.id === selected),
    [templates, selected],
  );

  const apply = async () => {
    if (!selected) {
      onSkip();
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await applyProjectTemplate(projectDir, selected);
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="agentz-tpl-overlay" onClick={onSkip}>
      <div className="agentz-tpl-dialog" onClick={(e) => e.stopPropagation()}>
        <h2>{t("projectTemplate.title")}</h2>
        <p className="agentz-tpl-sub">{t("projectTemplate.subtitle")}</p>
        {templates.length === 0 ? (
          <p className="agentz-tpl-empty">{t("projectTemplate.none")}</p>
        ) : (
          <ul className="agentz-tpl-list">
            {templates.map((tpl) => {
              const name = pickLocalized(tpl.name_zh, tpl.name, i18n.language);
              const desc = pickLocalized(tpl.description_zh, tpl.description, i18n.language);
              return (
                <li key={tpl.id}>
                  <label className={selected === tpl.id ? "active" : ""}>
                    <input
                      type="radio"
                      name="project-template"
                      checked={selected === tpl.id}
                      onChange={() => setSelected(tpl.id)}
                    />
                    <span className="agentz-tpl-name">{name}</span>
                    {desc && <span className="agentz-tpl-desc">{desc}</span>}
                  </label>
                </li>
              );
            })}
          </ul>
        )}
        {selectedTpl && (
          <p className="agentz-tpl-note">{t("projectTemplate.hint")}</p>
        )}
        {error && <p className="agentz-tpl-error">{error}</p>}
        <div className="agentz-tpl-actions">
          <button type="button" onClick={onSkip} disabled={busy}>
            {t("projectTemplate.skip")}
          </button>
          <button type="button" className="primary" onClick={() => void apply()} disabled={busy || !selected}>
            {busy ? t("common.loading") : t("projectTemplate.apply")}
          </button>
        </div>
      </div>
    </div>
  );
}
