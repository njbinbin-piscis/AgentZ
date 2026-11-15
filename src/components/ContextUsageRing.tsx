import { useTranslation } from "react-i18next";

/** How the most recent context compaction ended (kernel `CompactionStatusSnapshot`). */
export type CompactionOutcome = "ok" | "noop" | "fallback" | "failed";

export interface ContextCompactionStatus {
  /**
   * - `ok`: older history was summarised.
   * - `noop`: nothing could be compacted (the request is dominated by the
   *   system prompt and tool definitions, or history is already minimal).
   * - `fallback`: the summary could not be produced; older history was trimmed
   *   by rule instead.
   * - `failed`: compaction was needed and did not reduce the request.
   */
  outcome: CompactionOutcome;
  tier: string;
  beforeTokens: number;
  afterTokens: number;
  /** Compactions that took effect in this run. */
  count: number;
}

/** Snapshot mirroring the kernel's `AgentEvent::ContextUsage` payload. */
export interface ContextUsageSnapshot {
  estimatedInputTokens: number;
  totalInputBudget: number;
  triggerThreshold: number;
  cumulativeInputTokens: number;
  cumulativeOutputTokens: number;
  rollingSummaryVersion: number;
  autoCompactThreshold: number;
  /** Absent until the run has attempted a compaction. */
  lastCompaction?: ContextCompactionStatus | null;
}

function formatTokens(value: number | null | undefined): string {
  if (!value || value <= 0) return "0";
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`;
  return String(value);
}

const OUTCOME_KEYS: Record<CompactionOutcome, string> = {
  ok: "chat.contextCompactionOk",
  noop: "chat.contextCompactionNoop",
  fallback: "chat.contextCompactionFallback",
  failed: "chat.contextCompactionFailed",
};

/**
 * Compact ring indicator showing how full the context window is relative to the
 * input budget (mirrors openpiscis). Hidden until the first `ContextUsage`
 * event arrives.
 */
export default function ContextUsageRing({ usage }: { usage: ContextUsageSnapshot | null }) {
  const { t } = useTranslation();
  if (!usage || usage.triggerThreshold <= 0) return null;

  // Both the ring stroke and the label use totalInputBudget as the denominator.
  // triggerThreshold is only shown in the tooltip.
  const budgetPct =
    usage.totalInputBudget > 0
      ? Math.round((usage.estimatedInputTokens / usage.totalInputBudget) * 100)
      : 0;
  const pct = Math.min(100, budgetPct);
  // A full ring used to read "100%" whether the request just fit or was far
  // over budget; those need different reactions, so over-budget is marked.
  const over = budgetPct > 100;

  let color = "var(--accent, #6c63ff)";
  if (pct >= 100) color = "var(--danger, #e5484d)";
  else if (pct >= 80) color = "var(--warning, #f5a623)";

  const r = 7;
  const c = 2 * Math.PI * r;
  const dash = (pct / 100) * c;

  const compaction = usage.lastCompaction ?? null;
  const lines = [
    t("chat.contextRingTitle"),
    t("chat.contextRingEstimate", {
      estimated: formatTokens(usage.estimatedInputTokens),
      trigger: formatTokens(usage.triggerThreshold),
      budget: formatTokens(usage.totalInputBudget),
    }),
  ];
  if (over) lines.push(t("chat.contextRingOver", { pct: budgetPct }));
  lines.push(
    t("chat.contextRingCumulative", {
      input: formatTokens(usage.cumulativeInputTokens),
      output: formatTokens(usage.cumulativeOutputTokens),
    }),
  );
  if (compaction) {
    lines.push(t("chat.contextRingCompactions", { count: compaction.count }));
    lines.push(
      t("chat.contextRingLastCompaction", {
        status: t(OUTCOME_KEYS[compaction.outcome] ?? OUTCOME_KEYS.failed),
        before: formatTokens(compaction.beforeTokens),
        after: formatTokens(compaction.afterTokens),
      }),
    );
  }
  const tooltip = lines.join("\n");

  return (
    <span
      className={`agentz-context-ring${over ? " is-over" : ""}`}
      title={tooltip}
      aria-label={tooltip}
    >
      <svg width="18" height="18" viewBox="0 0 18 18">
        <circle cx="9" cy="9" r={r} fill="none" stroke="var(--border, #333)" strokeWidth="2.5" />
        <circle
          cx="9"
          cy="9"
          r={r}
          fill="none"
          stroke={color}
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeDasharray={`${dash} ${c}`}
          transform="rotate(-90 9 9)"
        />
      </svg>
      <span className="agentz-context-ring-label">{over ? "100%+" : `${pct}%`}</span>
    </span>
  );
}
