import { act, create } from "react-test-renderer";
import { describe, expect, it, vi } from "vitest";
import ModelPricesSection, { pricedModels, upsertPrice } from "./ModelPricesSection";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

const base = { provider: "openai", model: "gpt-5", input_per_million: 1, output_per_million: 4, currency: "USD" };

describe("ModelPricesSection helpers", () => {
  it("lists the default model and provider entries once per provider + model", () => {
    const models = pricedModels({ provider: "openai", model: "gpt-5" }, [
      { label: "Work GPT", provider: "OpenAI", model: "GPT-5" },
      { label: "DS", provider: "deepseek", model: "deepseek-chat" },
      { label: "empty", provider: "custom", model: "" },
    ]);
    expect(models.map((m) => `${m.provider}/${m.model}`)).toEqual(["openai/gpt-5", "deepseek/deepseek-chat"]);
  });

  it("replaces a price and drops it when both sides are cleared", () => {
    const other = { ...base, provider: "deepseek", model: "deepseek-chat" };
    const updated = upsertPrice([base, other], { ...base, input_per_million: 3 });
    expect(updated).toHaveLength(2);
    expect(updated.find((p) => p.model === "gpt-5")?.input_per_million).toBe(3);
    expect(upsertPrice(updated, { ...base, input_per_million: 0, output_per_million: 0 })).toEqual([other]);
  });
});

describe("ModelPricesSection", () => {
  it("writes edited prices back through onChange", async () => {
    const onChange = vi.fn();
    let root!: ReturnType<typeof create>;
    await act(async () => {
      root = create(
        <ModelPricesSection
          models={[{ provider: "openai", model: "gpt-5", label: "GPT-5" }]}
          prices={[]}
          onChange={onChange}
        />,
      );
    });
    const inputs = root.root.findAllByType("input");
    await act(async () => {
      inputs[0].props.onChange({ target: { value: "2.5" } });
    });
    expect(onChange).toHaveBeenCalledWith([
      { provider: "openai", model: "gpt-5", input_per_million: 2.5, output_per_million: 0, currency: "USD" },
    ]);
  });
});
