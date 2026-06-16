import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { listSlashCommands, type SlashCommandInfo } from "../../../services/tauri/slashCommands";

function pickLocalized(zh: string, en: string, lang: string): string {
  if (lang.startsWith("zh") && zh.trim()) return zh;
  return en.trim() ? en : zh;
}

export default function CommandsInstalledView() {
  const { t, i18n } = useTranslation();
  const [items, setItems] = useState<SlashCommandInfo[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(() => {
    setLoading(true);
    listSlashCommands()
      .then(setItems)
      .catch(() => setItems([]))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter((cmd) => {
      const id = (cmd.slash_id || cmd.id).toLowerCase();
      return (
        id.includes(q) ||
        cmd.name.toLowerCase().includes(q) ||
        cmd.description.toLowerCase().includes(q) ||
        cmd.description_zh.toLowerCase().includes(q)
      );
    });
  }, [items, query]);

  return (
    <div className="agentz-library-section">
      <div className="agentz-library-toolbar">
        <input
          type="search"
          className="agentz-library-search"
          placeholder={t("library.commandsSearch")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button type="button" className="agentz-library-refresh" onClick={refresh}>
          {t("common.refresh")}
        </button>
      </div>
      {loading ? (
        <p className="agentz-library-empty">{t("common.loading")}</p>
      ) : filtered.length === 0 ? (
        <p className="agentz-library-empty">{t("library.commandsEmpty")}</p>
      ) : (
        <ul className="agentz-command-list">
          {filtered.map((cmd) => {
            const id = cmd.slash_id || cmd.id;
            const desc = pickLocalized(cmd.description_zh, cmd.description, i18n.language);
            return (
              <li key={cmd.id} className="agentz-command-item">
                <div className="agentz-command-head">
                  <code>/{id}</code>
                  {cmd.source && <span className="agentz-command-source">{cmd.source}</span>}
                </div>
                {desc && <p className="agentz-command-desc">{desc}</p>}
                {cmd.argument_hint && (
                  <p className="agentz-command-hint">
                    {t("library.commandsArgs")}: {cmd.argument_hint}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
