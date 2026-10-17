import { useTranslation } from "react-i18next";
import type { ObserverBudgetSnapshot, TaskDetailSource } from "../../services/tauri/taskDetails";

/** Observed usage only; this component does not calculate remaining allowance. */
export default function BudgetObservation({
  source,
}: {
  source: TaskDetailSource<ObserverBudgetSnapshot>;
}) {
  const { i18n } = useTranslation();
  const zh = (i18n.resolvedLanguage ?? i18n.language ?? "").startsWith("zh");
  const text = (en: string, cn: string) => (zh ? cn : en);
  const labels: Record<string, string> = {
    missing: text("Not recorded", "暂无记录"),
    unknown: text("Unknown", "未知"),
    stale: text("Out of date", "已过期"),
    unavailable: text("Unavailable", "暂不可用"),
    unsupported: text("Not supported", "暂不支持"),
  };
  const budget = source.status === "available" ? source.value : null;
  return (
    <section aria-label={text("Budget observations", "预算观察记录")}>
      <strong>{text("Budget observations", "预算观察记录")}</strong>
      <p>
        {text(
          "Observer only. Budget enforcement is not connected; no remaining balance is available.",
          "仅供观察。预算执行控制尚未接通，暂无可用剩余额度。",
        )}
      </p>
      {!budget ? (
        <p>
          {text("Source: ", "来源：")}
          {labels[source.status] ?? text("Unknown", "未知")}
        </p>
      ) : (
        <>
          <dl>
            <dt>{text("Known input tokens", "已知输入 token")}</dt>
            <dd>{budget.known_input_tokens}</dd>
            <dt>{text("Known output tokens", "已知输出 token")}</dt>
            <dd>{budget.known_output_tokens}</dd>
            <dt>{text("Known active duration (ms)", "已知活跃时长（毫秒）")}</dt>
            <dd>{budget.known_active_ms}</dd>
            <dt>{text("Usage records", "用量记录数")}</dt>
            <dd>{budget.usage_count}</dd>
            <dt>{text("Records with unknown token usage", "token 用量未知的记录数")}</dt>
            <dd>{budget.unknown_token_records}</dd>
            <dt>{text("Records with unknown duration", "时长未知的记录数")}</dt>
            <dd>{budget.unknown_duration_records}</dd>
            <dt>{text("Records with unknown pricing", "价格未知的记录数")}</dt>
            <dd>{budget.unknown_price_records}</dd>
            <dt>{text("Configured token maximum", "配置的 token 上限")}</dt>
            <dd>{budget.policy.max_total_tokens ?? text("Not configured", "未配置")}</dd>
            <dt>{text("Configured active-duration maximum (ms)", "配置的活跃时长上限（毫秒）")}</dt>
            <dd>{budget.policy.max_active_ms ?? text("Not configured", "未配置")}</dd>
            <dt>{text("Configured cost maximum", "配置的费用上限")}</dt>
            <dd>
              {budget.policy.max_cost
                ? `${budget.policy.max_cost.micro_units} ${budget.policy.max_cost.currency} ${text("micro-units", "百万分之一单位")}`
                : text("Not configured", "未配置")}
            </dd>
          </dl>
          <p>
            {text(
              "Known costs by currency (micro-units)",
              "按币种统计的已知费用（百万分之一单位）",
            )}
          </p>
          {Object.keys(budget.known_cost_micro_units).length === 0 ? (
            <p>{text("No known cost observations", "暂无已知费用记录")}</p>
          ) : (
            <ul>
              {Object.entries(budget.known_cost_micro_units).map(([currency, cost]) => (
                <li key={currency}>
                  {currency}: {cost}
                </li>
              ))}
            </ul>
          )}
          {budget.observed_limits_exceeded.length > 0 && (
            <p>
              {text("Observed configured-limit exceedances: ", "观察到已超过配置上限：")}
              {budget.observed_limits_exceeded.join(", ")}
            </p>
          )}
          <small>
            {text(
              "Known totals can be incomplete when records contain unknown usage or prices. Configured maxima are not an enforced allowance.",
              "存在用量或价格未知记录时，已知汇总可能不完整。配置上限不代表已生效的额度控制。",
            )}
          </small>
        </>
      )}
    </section>
  );
}
