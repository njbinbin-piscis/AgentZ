/** User-entered list price of one provider + model, per million tokens. */
export interface ModelPrice {
  provider: string;
  model: string;
  input_per_million: number;
  output_per_million: number;
  /** ISO 4217 code; empty means USD. */
  currency: string;
}

const SYMBOLS: Record<string, string> = { USD: "$", CNY: "¥", EUR: "€", JPY: "¥", GBP: "£" };

export function findModelPrice(
  prices: ModelPrice[] | undefined,
  provider: string,
  model: string,
): ModelPrice | undefined {
  if (!prices || !model) return undefined;
  const p = provider.trim().toLowerCase();
  const m = model.trim().toLowerCase();
  return prices.find((x) => x.provider.trim().toLowerCase() === p && x.model.trim().toLowerCase() === m);
}

/** A price with both sides at zero is treated as "not configured". */
export function hasPrice(price: ModelPrice | undefined): price is ModelPrice {
  return !!price && (price.input_per_million > 0 || price.output_per_million > 0);
}

/** Formatted cost of the given token counts, or null when no price is configured. */
export function formatTokenCost(
  inputTokens: number,
  outputTokens: number,
  price: ModelPrice | undefined,
): string | null {
  if (!hasPrice(price)) return null;
  const amount = (inputTokens * price.input_per_million + outputTokens * price.output_per_million) / 1_000_000;
  const code = (price.currency || "USD").toUpperCase();
  const symbol = SYMBOLS[code] ?? `${code} `;
  if (amount < 0.001 && inputTokens + outputTokens > 0) return `<${symbol}0.001`;
  return `${symbol}${amount.toFixed(3)}`;
}

export interface ModelUsageRow {
  model: string;
  input_tokens: number;
  output_tokens: number;
}

export interface UsageSummaryView {
  inputTokens: number;
  outputTokens: number;
  /** Cost of the priced models, or null when none has a price. */
  cost: string | null;
  /** True when some used models have no configured price (cost is a lower bound). */
  partial: boolean;
}

function amountLabel(amount: number, code: string): string {
  const symbol = SYMBOLS[code] ?? `${code} `;
  if (amount > 0 && amount < 0.001) return `<${symbol}0.001`;
  return `${symbol}${amount.toFixed(3)}`;
}

/** Sum per-model usage rows; each row is priced by its model name (any provider). */
export function summarizeUsage(rows: ModelUsageRow[], prices: ModelPrice[] | undefined): UsageSummaryView {
  let inputTokens = 0;
  let outputTokens = 0;
  let partial = false;
  const byCurrency = new Map<string, number>();
  for (const r of rows) {
    inputTokens += r.input_tokens;
    outputTokens += r.output_tokens;
    const m = r.model.trim().toLowerCase();
    const price = prices?.find((x) => x.model.trim().toLowerCase() === m && hasPrice(x));
    if (!price) {
      partial = true;
      continue;
    }
    const code = (price.currency || "USD").toUpperCase();
    const amount = (r.input_tokens * price.input_per_million + r.output_tokens * price.output_per_million) / 1_000_000;
    byCurrency.set(code, (byCurrency.get(code) ?? 0) + amount);
  }
  const parts = [...byCurrency.entries()].map(([code, amount]) => amountLabel(amount, code));
  return { inputTokens, outputTokens, cost: parts.length ? parts.join(" + ") : null, partial };
}
