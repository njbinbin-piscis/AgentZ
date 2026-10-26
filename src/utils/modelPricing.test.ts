import { describe, expect, it } from "vitest";
import { findModelPrice, formatTokenCost, hasPrice, type ModelPrice } from "./modelPricing";

const price = (over: Partial<ModelPrice> = {}): ModelPrice => ({
  provider: "deepseek",
  model: "deepseek-chat",
  input_per_million: 2,
  output_per_million: 8,
  currency: "CNY",
  ...over,
});

describe("modelPricing", () => {
  it("matches provider and model case-insensitively", () => {
    const prices = [price(), price({ provider: "openai", model: "gpt-5" })];
    expect(findModelPrice(prices, "DeepSeek", "DeepSeek-Chat")?.currency).toBe("CNY");
    expect(findModelPrice(prices, "deepseek", "other")).toBeUndefined();
    expect(findModelPrice(prices, "deepseek", "")).toBeUndefined();
    expect(findModelPrice(undefined, "deepseek", "deepseek-chat")).toBeUndefined();
  });

  it("computes cost from the configured prices and currency", () => {
    expect(formatTokenCost(500_000, 100_000, price())).toBe("¥1.800");
    expect(formatTokenCost(1_000_000, 0, price({ currency: "" }))).toBe("$2.000");
    expect(formatTokenCost(1000, 1000, price({ currency: "CHF" }))).toBe("CHF 0.010");
  });

  it("shows no cost without a configured price", () => {
    expect(formatTokenCost(1000, 1000, undefined)).toBeNull();
    expect(formatTokenCost(1000, 1000, price({ input_per_million: 0, output_per_million: 0 }))).toBeNull();
    expect(hasPrice(price({ input_per_million: 0 }))).toBe(true);
  });

  it("marks tiny costs as below the display threshold", () => {
    expect(formatTokenCost(10, 10, price())).toBe("<¥0.001");
  });
});

import { summarizeUsage } from "./modelPricing";
describe("summarizeUsage", () => {
  const prices = [{ provider: "p", model: "A", input_per_million: 1, output_per_million: 2, currency: "USD" }];
  it("prices rows by model name and flags unpriced models", () => {
    const s = summarizeUsage(
      [
        { model: "a", input_tokens: 1_000_000, output_tokens: 500_000 },
        { model: "b", input_tokens: 10, output_tokens: 0 },
      ],
      prices,
    );
    expect(s.cost).toBe("$2.000");
    expect(s.partial).toBe(true);
    expect(s.inputTokens).toBe(1_000_010);
  });
  it("returns null cost with no price", () => {
    expect(summarizeUsage([{ model: "x", input_tokens: 5, output_tokens: 5 }], []).cost).toBeNull();
  });
});
