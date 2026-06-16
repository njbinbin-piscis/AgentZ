import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listInstalledSkills } from "../../../services/tauri/workbench";
import {
  clawHubApi,
  type ClawHubSkill,
  type SkillRegistry,
} from "../../../services/tauri/clawhub";

const REGISTRIES: SkillRegistry[] = ["skillhub", "clawhub"];

export default function SkillsDiscoverView() {
  const { t } = useTranslation();
  const [registry, setRegistry] = useState<SkillRegistry>("skillhub");
  const [installedSlugs, setInstalledSlugs] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ClawHubSkill[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [busySlug, setBusySlug] = useState<string | null>(null);

  const refreshInstalled = useCallback(async () => {
    try {
      const skills = await listInstalledSkills();
      setInstalledSlugs(new Set(skills.map((s) => s.slug)));
    } catch {
      setInstalledSlugs(new Set());
    }
  }, []);

  useEffect(() => {
    void refreshInstalled();
  }, [refreshInstalled]);

  const doSearch = useCallback(async () => {
    setSearching(true);
    setSearchError(null);
    try {
      const res = await clawHubApi.search(query, 20, registry);
      setResults(res.items);
    } catch (e) {
      setSearchError(String(e));
      setResults([]);
    } finally {
      setSearching(false);
    }
  }, [query, registry]);

  useEffect(() => {
    void doSearch();
  }, [doSearch]);

  const doInstall = useCallback(
    async (skill: ClawHubSkill) => {
      setBusySlug(skill.slug);
      setSearchError(null);
      try {
        await clawHubApi.install(skill.slug, skill.version, registry);
        await refreshInstalled();
      } catch (e) {
        setSearchError(String(e));
      } finally {
        setBusySlug(null);
      }
    },
    [refreshInstalled, registry],
  );

  return (
    <div className="agentz-settings-tabpanel">
      <section className="agentz-settings-section">
        <h3>{t("library.discoverSkillsTitle")}</h3>
        <div className="agentz-skill-discover-tabs" role="tablist" aria-label={t("skills.registryTabs")}>
          {REGISTRIES.map((id) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={registry === id}
              className={`agentz-library-view${registry === id ? " active" : ""}`}
              onClick={() => setRegistry(id)}
            >
              {t(`skills.registry.${id}`)}
            </button>
          ))}
        </div>
        <p className="agentz-settings-hint">
          {registry === "skillhub" ? t("skills.skillhubHint") : t("skills.clawhubHint")}
        </p>
        <div className="agentz-wb-search">
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void doSearch();
            }}
            placeholder={t("skills.searchPlaceholder")}
          />
          <button type="button" onClick={() => void doSearch()} disabled={searching}>
            {searching ? t("skills.searching") : t("skills.search")}
          </button>
        </div>
        {searchError && <div className="agentz-settings-error">{searchError}</div>}
        <div className="agentz-wb-list">
          {results.length === 0 && !searching && (
            <div className="agentz-wb-empty">{t("library.empty")}</div>
          )}
          {results.map((r) => {
            const already = installedSlugs.has(r.slug);
            return (
              <div key={`${registry}:${r.slug}`} className="agentz-wb-row">
                <div className="agentz-wb-info">
                  <strong>{r.name}</strong>
                  <span className="agentz-wb-meta">
                    {r.slug}
                    {r.version ? ` · v${r.version}` : ""}
                    {r.stars ? ` · ★${r.stars}` : ""}
                  </span>
                  {r.description && <span className="agentz-wb-desc">{r.description}</span>}
                </div>
                <div className="agentz-wb-actions">
                  <button
                    type="button"
                    disabled={already || busySlug === r.slug}
                    onClick={() => void doInstall(r)}
                  >
                    {already
                      ? t("skills.installed")
                      : busySlug === r.slug
                        ? t("skills.installing")
                        : t("skills.install")}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </section>
    </div>
  );
}
