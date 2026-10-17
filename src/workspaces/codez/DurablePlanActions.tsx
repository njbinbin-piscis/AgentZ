import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { applyTaskPlanIntent, queryTaskPlan, type DurablePlan } from "../../services/tauri/chatPlan";
import { useProjectScope } from "../../services/tauri/projectScope";
import "./DurablePlanActions.css";

/** Explicit goal actions are separate from ordinary send/continue. */
export default function DurablePlanActions({ projectDir, sessionId, busy, onPlanChanged }: {
  projectDir: string | null;
  sessionId: string | null;
  busy: boolean;
  onPlanChanged: (plan: DurablePlan) => void;
}) {
  const { t } = useTranslation();
  const [plan, setPlan] = useState<DurablePlan | null>(null);
  const [objective, setObjective] = useState("");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const sequence = useRef(0);
  const invalidate = useCallback(() => ++sequence.current, []);
  const scope = useProjectScope(projectDir);
  useEffect(() => {
    const request = invalidate();
    setPlan(null);
    setObjective("");
    setError(null);
    if (!scope || !sessionId || busy) return;
    void queryTaskPlan(scope.handle, sessionId).then((result) => {
      if (sequence.current !== request) return;
      setPlan(result.current);
      if (result.current) onPlanChanged(result.current);
    }).catch((cause: unknown) => {
      if (sequence.current === request) setError(String(cause));
    });
    return () => { invalidate(); };
  }, [scope, sessionId, busy, onPlanChanged, invalidate, refresh]);

  async function apply(kind: "replace" | "cancel") {
    if (!scope || !sessionId || !plan || busy || pending) return;
    const request = ++sequence.current;
    setPending(true);
    setError(null);
    try {
      const next = await applyTaskPlanIntent(scope.handle, sessionId,
        { id: plan.id, revision: plan.revision },
        kind === "cancel" ? { kind } : { kind, objective });
      if (sequence.current !== request) return;
      setPlan(next);
      setObjective("");
      onPlanChanged(next);
    } catch (cause: unknown) {
      if (sequence.current === request) setError(String(cause));
    } finally {
      setPending(false);
    }
  }
  if (!scope || !sessionId) return null;
  return <div className="agentz-plan-actions">
    <button disabled={busy || pending} onClick={() => setRefresh((value) => value + 1)}>
      {t("chat.durablePlan.refresh", { defaultValue: "Refresh plan" })}
    </button>
    {plan && <>
      <div>{plan.objective}</div>
      <small>{t(`chat.durablePlan.status.${plan.status}`, { defaultValue: plan.status })}</small>
      <input aria-label={t("chat.durablePlan.newGoal", { defaultValue: "New task goal" })}
        value={objective} onChange={(event) => setObjective(event.target.value)} disabled={busy || pending} />
      <button disabled={busy || pending || !objective.trim()} onClick={() => void apply("replace")}>
        {t("chat.durablePlan.replace", { defaultValue: "Replace goal" })}
      </button>
      <button disabled={busy || pending || plan.status === "cancelled" || plan.status === "replaced"}
        onClick={() => void apply("cancel")}>
        {t("chat.durablePlan.cancel", { defaultValue: "Cancel plan" })}
      </button>
    </>}
    {error && <div role="alert">{error}</div>}
  </div>;
}
