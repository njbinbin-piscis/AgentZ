import { useState } from "react";
import { useTranslation } from "react-i18next";

export const ONBOARDING_DONE_KEY = "agentz.onboardingDone.v1";

export type OnboardingStep = 1 | 2 | 3;

export interface OnboardingModalProps {
  open: boolean;
  hasConfiguredModel: boolean;
  currentMode: "workz" | "codez" | "botz";
  onOpenSettings: () => void;
  onSelectMode: (mode: "workz" | "codez") => void;
  onFinish: () => void;
  onDismiss: () => void;
}

export function isOnboardingDone(): boolean {
  try {
    if (typeof localStorage === "undefined") return false;
    return localStorage.getItem(ONBOARDING_DONE_KEY) === "1";
  } catch {
    return false;
  }
}

export function markOnboardingDone(): void {
  try {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(ONBOARDING_DONE_KEY, "1");
  } catch {
    /* ignore */
  }
}

export default function OnboardingModal({
  open,
  hasConfiguredModel,
  currentMode,
  onOpenSettings,
  onSelectMode,
  onFinish,
  onDismiss,
}: OnboardingModalProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<OnboardingStep>(1);

  if (!open) return null;

  const handleComplete = () => {
    markOnboardingDone();
    onFinish();
  };

  const handleSkip = () => {
    markOnboardingDone();
    onDismiss();
  };

  return (
    <div className="agentz-onboarding-backdrop" data-testid="onboarding-modal">
      <div className="agentz-onboarding-card" role="dialog" aria-modal="true">
        <div className="agentz-onboarding-header">
          <div className="agentz-onboarding-steps" data-testid="onboarding-step-indicator">
            <span className={step === 1 ? "active" : ""}>1 · {t("onboarding.step1Short")}</span>
            <span>→</span>
            <span className={step === 2 ? "active" : ""}>2 · {t("onboarding.step2Short")}</span>
            <span>→</span>
            <span className={step === 3 ? "active" : ""}>3 · {t("onboarding.step3Short")}</span>
          </div>
          <button
            type="button"
            data-testid="onboarding-skip-btn"
            className="agentz-onboarding-skip"
            onClick={handleSkip}
          >
            {t("onboarding.skip")}
          </button>
        </div>

        {step === 1 && (
          <div className="agentz-onboarding-body" data-testid="onboarding-step-1">
            <h3>{t("onboarding.step1Title")}</h3>
            <p>{t("onboarding.step1Desc")}</p>
            <div className="agentz-onboarding-status">
              <span
                className={`agentz-onboarding-badge ${hasConfiguredModel ? "ok" : "warn"}`}
                data-testid="onboarding-model-status"
              >
                {hasConfiguredModel
                  ? t("onboarding.modelConfigured")
                  : t("onboarding.modelNeedsSetup")}
              </span>
              <button
                type="button"
                data-testid="onboarding-open-settings-btn"
                className="agentz-onboarding-secondary-btn"
                onClick={onOpenSettings}
              >
                {t("onboarding.configureModelBtn")}
              </button>
            </div>
            <div className="agentz-onboarding-footer">
              <button
                type="button"
                data-testid="onboarding-next-1"
                className="agentz-onboarding-primary-btn"
                onClick={() => setStep(2)}
              >
                {t("onboarding.nextStep")}
              </button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="agentz-onboarding-body" data-testid="onboarding-step-2">
            <h3>{t("onboarding.step2Title")}</h3>
            <p>{t("onboarding.step2Desc")}</p>
            <div className="agentz-onboarding-modes">
              <button
                type="button"
                data-testid="onboarding-mode-workz"
                className={`agentz-onboarding-mode-card ${currentMode === "workz" ? "selected" : ""}`}
                onClick={() => onSelectMode("workz")}
              >
                <strong>WorkZ · {t("onboarding.modeWorkzTitle")}</strong>
                <span>{t("onboarding.modeWorkzDesc")}</span>
              </button>
              <button
                type="button"
                data-testid="onboarding-mode-codez"
                className={`agentz-onboarding-mode-card ${currentMode === "codez" ? "selected" : ""}`}
                onClick={() => onSelectMode("codez")}
              >
                <strong>CodeZ · {t("onboarding.modeCodezTitle")}</strong>
                <span>{t("onboarding.modeCodezDesc")}</span>
              </button>
            </div>
            <div className="agentz-onboarding-footer">
              <button
                type="button"
                className="agentz-onboarding-secondary-btn"
                onClick={() => setStep(1)}
              >
                {t("onboarding.prevStep")}
              </button>
              <button
                type="button"
                data-testid="onboarding-next-2"
                className="agentz-onboarding-primary-btn"
                onClick={() => setStep(3)}
              >
                {t("onboarding.nextStep")}
              </button>
            </div>
          </div>
        )}

        {step === 3 && (
          <div className="agentz-onboarding-body" data-testid="onboarding-step-3">
            <h3>{t("onboarding.step3Title")}</h3>
            <p>{t("onboarding.step3Desc")}</p>
            <div className="agentz-onboarding-footer">
              <button
                type="button"
                className="agentz-onboarding-secondary-btn"
                onClick={() => setStep(2)}
              >
                {t("onboarding.prevStep")}
              </button>
              <button
                type="button"
                data-testid="onboarding-finish-btn"
                className="agentz-onboarding-primary-btn"
                onClick={handleComplete}
              >
                {t("onboarding.startNow")}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
