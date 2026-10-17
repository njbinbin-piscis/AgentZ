import { useTranslation } from "react-i18next";
import { firstRunSteps, type FirstRunStepId } from "./firstRun";
import "./FirstRunChecklist.css";

interface FirstRunChecklistProps {
  projectReady: boolean;
  /** null while the settings check is still in flight. */
  configured: boolean | null;
  firstTaskDone: boolean;
  onOpenFolder: () => void;
  onOpenSettings: () => void;
  onTryExample: () => void;
}

const STEP_LABEL_KEY: Record<FirstRunStepId, string> = {
  project: "chat.firstRunStepProject",
  api_key: "chat.firstRunStepKey",
  first_task: "chat.firstRunStepTask",
};

export default function FirstRunChecklist({ projectReady, configured, firstTaskDone, onOpenFolder, onOpenSettings, onTryExample }: FirstRunChecklistProps) {
  const { t } = useTranslation();
  const steps = firstRunSteps({ projectReady, configured, firstTaskDone });
  const keyMissing = configured === false;

  return (
    <section className="agentz-first-run" aria-label={t("chat.firstRunTitle")}>
      <div className="agentz-first-run-title">{t("chat.firstRunTitle")}</div>
      <ol className="agentz-first-run-steps">
        {steps.map((step) => (
          <li key={step.id} data-status={step.status}>
            <span aria-hidden="true">{step.status === "done" ? "✓" : "○"}</span> {t(STEP_LABEL_KEY[step.id])}
          </li>
        ))}
      </ol>
      {!projectReady && (
        <button type="button" className="agentz-first-run-action" onClick={onOpenFolder}>
          {t("app.openFolder")}
        </button>
      )}
      {keyMissing && (
        <button type="button" className="agentz-first-run-action" onClick={onOpenSettings}>
          {t("chat.firstRunOpenSettings")}
        </button>
      )}
      <button type="button" className="agentz-first-run-action" disabled={!projectReady} onClick={onTryExample}>
        {t("chat.firstRunTryExample")}
      </button>
      <p>{t("chat.firstRunExampleHint")}</p>
    </section>
  );
}
