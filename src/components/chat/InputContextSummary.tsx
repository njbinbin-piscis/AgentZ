import { useTranslation } from "react-i18next";

export interface InputContextSummaryProps {
  authorityProject: string | null;
  workspace: string | null;
  model: string | null;
  modelSource: "selected" | "default" | "unknown";
  policy: string | null;
  mode: string;
  state: "idle" | "running" | "queued" | "approval" | "error";
  latestAction?: { name: string; status: "running" | "done" | "error" } | null;
  optionalCount?: number;
}
/** Pure configured context display. It neither verifies scope nor grants permission. */
export default function InputContextSummary(props: InputContextSummaryProps) {
  const { i18n } = useTranslation();
  const zh = (i18n.resolvedLanguage ?? i18n.language ?? "").startsWith("zh");
  const text = (en: string, cn: string) => zh ? cn : en;
  const known = (value: string | null) => value?.trim() ? value : text("Unknown", "未知");
  const statuses = {
    idle: text("Idle", "空闲"), running: text("Running", "执行中"),
    queued: text("Queued locally", "本地待发"), approval: text("Awaiting approval", "等待审批"),
    error: text("Error", "错误"),
  };
  const modelSource = props.modelSource === "selected" ? text("Selected", "已选择")
    : props.modelSource === "default" ? text("Default", "默认") : text("Source unknown", "来源未知");
  const model = props.model?.trim() ? props.model
    : props.modelSource === "selected" ? text("Not selected", "未选择")
      : props.modelSource === "default" ? text("Default not resolved", "默认模型未解析") : text("Unknown", "未知");
  return <section className="input-context-summary" aria-label={text("Input context", "输入上下文")}>
    <div className="input-context-summary-primary">
      <span>{text("Project", "项目")}: {known(props.authorityProject)}</span>{" · "}
      <span>{text("Workspace", "工作区")}: {known(props.workspace)}</span>{" · "}
      <span>{text("Model", "模型")}: {model} ({modelSource})</span>{" · "}
      <span role="status">{text("State", "状态")}: {statuses[props.state]}</span>
      {props.latestAction && <span>{text("Latest action", "最近动作")}: {known(props.latestAction.name)} ({props.latestAction.status === "running" ? text("Running", "执行中") : props.latestAction.status === "error" ? text("Failed", "失败") : text("Completed", "已完成")})</span>}
      {props.optionalCount !== undefined && <span>{" · "}{text("Optional context items", "可选上下文项")}: {Number.isSafeInteger(props.optionalCount) && props.optionalCount >= 0 ? props.optionalCount : text("Unknown", "未知")}</span>}
    </div>
    <details>
      <summary>{text("Configuration details", "配置详情")}</summary>
      <p>{text("Mode", "模式")}: {known(props.mode)}</p>
      <p>{text("Configured policy", "配置的策略")}: {known(props.policy)}. {text("Configuration does not grant permission; each action still requires its applicable authorization.", "配置不代表已授予权限；每个动作仍须满足相应授权。")}</p>
      <p>{text("Workspace shown here is configured context, not host verification of an execution environment.", "此处工作区是配置上下文，不代表宿主已验证执行环境。")}</p>
    </details>
  </section>;
}
