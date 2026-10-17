import { useTranslation } from "react-i18next";
import type { CurrentTaskRun } from "../../services/tauri/runDiscovery";
import type { TaskDetailSource } from "../../services/tauri/taskDetails";
import type { ArtifactList } from "../../services/tauri/taskArtifacts";

export type ArtifactObservation = { status: "available"; value: ArtifactList }
  | { status: "missing" | "unknown" | "stale" | "unavailable" | "unsupported" };
export interface DeliverySummaryProps {
  run: TaskDetailSource<CurrentTaskRun>;
  artifacts: ArtifactObservation;
  onReviewChanges?: () => void;
  onOpenArtifacts?: () => void;
}
/** Observation only: execution, attachments and receipt sources are distinct. */
export default function DeliverySummary({ run, artifacts, onReviewChanges, onOpenArtifacts }: DeliverySummaryProps) {
  const { i18n } = useTranslation();
  const zh = (i18n.resolvedLanguage ?? i18n.language ?? "").startsWith("zh");
  const text = (en: string, cn: string) => zh ? cn : en;
  const states = {
    queued: text("Queued", "已排队"), running: text("Running", "执行中"),
    cancelling: text("Stopping", "正在停止"), succeeded: text("Succeeded", "执行成功"),
    failed: text("Failed", "执行失败"), cancelled: text("Cancelled", "已取消"),
    interrupted: text("Interrupted", "已中断"), missing: text("Not recorded", "暂无记录"),
    unknown: text("Unknown", "未知"), stale: text("Out of date", "已过期"),
    unavailable: text("Unavailable", "暂不可用"), unsupported: text("Not supported", "暂不支持"),
  };
  const execution = run.status === "available" ? run.value.execution : run.status;
  const decisions = {
    accepted: text("Accepted", "已接受"), rejected: text("Rejected", "已拒绝"),
    awaiting_review: text("Awaiting review", "等待评审"), not_ready: text("Not ready", "尚未就绪"),
  };
  // Count this returned page only. Store-missing is not an available complete list.
  const list = artifacts.status === "available" && !artifacts.value.store_missing ? artifacts.value : null;
  const count = list ? new Set(list.page.versions.map(v => v.artifact_id)).size : null;
  const validationCount = list?.page.versions.filter(v => v.kind === "validation").length;
  return <section className="delivery-summary" aria-label={text("Delivery summary", "交付摘要")}>
    <div className="delivery-summary-primary">
      <strong>{text("Execution", "执行")}: {states[execution]}</strong>{" · "}
      {list ? <span>{text("Artifacts shown", "本页产物")}: {count}; {text("retained versions shown", "本页保留版本")}: {list.page.versions.length}{list.page.has_more ? text(" (more available)", "（还有更多）") : ""}</span>
        : <span>{text("Artifacts", "产物")}: {states[artifacts.status === "available" ? "missing" : artifacts.status]}</span>}
    </div>
    <p>{text("Run delivery decision", "执行交付决策")}: {run.status === "available" ? decisions[run.value.delivery] : states[run.status]}. {text("A Run decision does not establish acceptance of a specific artifact version.", "执行交付决策不代表某个产物版本已验收。")}</p>
    <p>{text("Artifact-version acceptance", "产物版本验收")}: {text("No receipt provided", "未提供回执")}{" · "}
      {text("Applied", "应用")}: {text("No receipt provided", "未提供回执")}{" · "}
      {text("Published", "发布")}: {text("No receipt provided", "未提供回执")}</p>
    <p>{text("Validation attachments shown", "本页验证附件")}: {validationCount ?? text("Unknown", "未知")}. {text("An attachment records evidence; it does not establish a passed validation or acceptance.", "附件仅记录证据，不代表验证或验收已通过。")}</p>
    <small>{text("Execution and artifacts were observed separately; this is not an atomic snapshot.", "执行与产物分别读取，此处不是原子快照。")}</small>
    <div>
      {onReviewChanges && <button type="button" onClick={onReviewChanges}>{text("View changes", "查看变更")}</button>}
      {onOpenArtifacts && <button type="button" onClick={onOpenArtifacts}>{text("View artifacts", "查看产物")}</button>}
    </div>
  </section>;
}
