import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useProjectScope } from "../../services/tauri/projectScope";
import { readTaskDetails, type TaskDetails } from "../../services/tauri/taskDetails";
import { coalesce, FOCUS_REFRESH_GAP_MS } from "../../utils/coalesce";
import BudgetObservation from "./BudgetObservation";

const labels: Record<string, [string, string]> = {
  queued: ["Queued", "已排队"], running: ["Running", "执行中"],
  cancelling: ["Stopping", "正在停止"], succeeded: ["Succeeded", "执行成功"],
  failed: ["Failed", "执行失败"], cancelled: ["Cancelled", "已取消"],
  interrupted: ["Interrupted", "已中断"], active: ["Active", "进行中"],
  completed: ["Completed", "已完成"], available: ["Available", "可用"],
  missing: ["Not recorded", "暂无记录"], unsupported: ["Not supported", "暂不支持"],
  unavailable: ["Unavailable", "暂不可用"], stale: ["Out of date", "已过期"],
  unknown: ["Unknown", "未知"],
};

/** Passive task summary. It grants no permission and never resumes execution. */
export default function TaskDetailsPanel({ projectDir, sessionId, busy }: {
  projectDir: string | null; sessionId: string | null; busy: boolean;
}) {
  const { i18n } = useTranslation();
  const zh = (i18n.resolvedLanguage ?? i18n.language ?? "").startsWith("zh");
  const text = (en: string, cn: string) => zh ? cn : en;
  const status = (value: string) => Object.prototype.hasOwnProperty.call(labels, value)
    ? labels[value][zh ? 1 : 0] : text("Unknown", "未知");
  const scope = useProjectScope(projectDir);
  const key = JSON.stringify([projectDir, scope?.handle ?? null, sessionId]);
  const [view, setView] = useState<{ key: string; data: TaskDetails | null; loading: boolean; failed: boolean }>({
    key, data: null, loading: Boolean(scope && sessionId), failed: false,
  });
  const generation = useRef(0);
  const refresh = useRef<() => void>(() => {});
  // Mask old scope during render, before effect cleanup and asynchronous replies.
  const visible = view.key === key ? view : { data: null, loading: Boolean(scope && sessionId), failed: false };
  useEffect(() => {
    const current = ++generation.current;
    let disposed = false;
    let inFlight = false;
    let pending = false;
    setView({ key, data: null, loading: Boolean(scope && sessionId), failed: false });
    const load = async (): Promise<void> => {
      if (disposed || !scope || !sessionId) return;
      if (inFlight) { pending = true; return; }
      inFlight = true;
      setView({ key, data: null, loading: true, failed: false });
      try {
        const data = await readTaskDetails(scope.handle, sessionId);
        if (!disposed && generation.current === current) setView({ key, data, loading: false, failed: false });
      } catch {
        if (!disposed && generation.current === current) setView({ key, data: null, loading: false, failed: true });
      } finally {
        inFlight = false;
        if (pending && !disposed && generation.current === current) {
          pending = false;
          void load();
        }
      }
    };
    refresh.current = () => { void load(); };
    // Focus can fire in bursts (the window regains focus, then a child process steals it);
    // coalesce them so each burst costs one reload.
    const focusLoad = coalesce(() => void load(), FOCUS_REFRESH_GAP_MS);
    window.addEventListener("focus", focusLoad.call);
    void load();
    return () => {
      disposed = true;
      focusLoad.cancel();
      window.removeEventListener("focus", focusLoad.call);
      refresh.current = () => {};
    };
  }, [key, scope, sessionId, busy]);
  if (!sessionId || !projectDir) return null;
  const data = visible.data;
  return <details className="task-details-panel">
    <summary>{text("Task summary", "任务概况")}</summary>
    <button type="button" disabled={!scope || visible.loading} onClick={() => refresh.current()}>
      {text("Refresh", "刷新")}
    </button>
    {!scope ? <p>{text("Project access is unavailable.", "项目访问暂不可用。")}</p>
      : visible.loading ? <p>{text("Loading…", "正在读取…")}</p>
      : visible.failed ? <p role="alert">{text("Could not read task details. Refresh to retry.", "任务详情读取失败，请刷新重试。")}</p>
      : data ? <>
        <p>{text("Run", "执行")}: {data.sources.run.status === "available" ? status(data.sources.run.value.execution) : status(data.sources.run.status)}</p>
        <p>{text("Plan", "计划")}: {data.sources.plan.status === "available" ? <>
          {status(data.sources.plan.value.status)} · {data.sources.plan.value.items.filter(item => item.state === "done").length}/{data.sources.plan.value.items.length}
        </> : status(data.sources.plan.status)}</p>
        <p>{text("Environment", "环境")}: {status(data.sources.environment.status)}</p>
        <p>{text("Events", "事件")}: {status(data.sources.events.status)} · {text("Artifacts", "产物")}: {status(data.sources.artifacts.status)}</p>
        <BudgetObservation source={data.sources.budget} />
        <small>{text("Sources were read separately; this is not an atomic snapshot.", "各来源分别读取，此处不是原子快照。")}</small>
      </> : null}
  </details>;
}
