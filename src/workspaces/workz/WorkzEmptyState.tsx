import { useTranslation } from "react-i18next";

export interface WorkzStarterScenario {
  id: "files" | "docs" | "web" | "code";
  titleKey: string;
  descKey: string;
  promptKey: string;
  recommendIsolate: boolean;
}

export const WORKZ_STARTER_SCENARIOS: WorkzStarterScenario[] = [
  {
    id: "files",
    titleKey: "workz.starterFilesTitle",
    descKey: "workz.starterFilesDesc",
    promptKey: "workz.starterFilesPrompt",
    recommendIsolate: false,
  },
  {
    id: "docs",
    titleKey: "workz.starterDocsTitle",
    descKey: "workz.starterDocsDesc",
    promptKey: "workz.starterDocsPrompt",
    recommendIsolate: true,
  },
  {
    id: "web",
    titleKey: "workz.starterWebTitle",
    descKey: "workz.starterWebDesc",
    promptKey: "workz.starterWebPrompt",
    recommendIsolate: false,
  },
  {
    id: "code",
    titleKey: "workz.starterCodeTitle",
    descKey: "workz.starterCodeDesc",
    promptKey: "workz.starterCodePrompt",
    recommendIsolate: true,
  },
];

export interface WorkzEmptyStateProps {
  title: string;
  subtitle: string;
  onSelectScenario: (scenario: WorkzStarterScenario) => void;
}

export default function WorkzEmptyState({
  title,
  subtitle,
  onSelectScenario,
}: WorkzEmptyStateProps) {
  const { t } = useTranslation();

  return (
    <div className="agentz-workz-empty" data-testid="workz-empty-state">
      <div className="agentz-workz-title">{title}</div>
      <p className="agentz-workz-sub">{subtitle}</p>
      <div className="agentz-workz-starter-grid" data-testid="workz-starter-grid">
        {WORKZ_STARTER_SCENARIOS.map((sc) => (
          <button
            key={sc.id}
            type="button"
            data-testid={`workz-starter-${sc.id}`}
            className="agentz-workz-starter-card"
            onClick={() => onSelectScenario(sc)}
          >
            <div className="agentz-workz-starter-title">
              {t(sc.titleKey)}
            </div>
            <div className="agentz-workz-starter-desc">
              {t(sc.descKey)}
            </div>
            <span className="agentz-workz-starter-cta">
              {t("workz.starterUsePrompt")} →
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
