import { useTranslation } from "react-i18next";
import { findModelPrice, hasPrice, type ModelPrice } from "../../../utils/modelPricing";

export interface PricedModel {
  provider: string;
  model: string;
  label: string;
}

const CURRENCIES = ["USD", "CNY", "EUR", "JPY", "GBP"];

/** The distinct provider + model pairs a price can be set for. */
export function pricedModels(
  main: { provider: string; model: string },
  providers: Array<{ label: string; provider: string; model: string }>,
): PricedModel[] {
  const seen = new Set<string>();
  const out: PricedModel[] = [];
  const add = (provider: string, model: string, label: string) => {
    const key = `${provider.trim().toLowerCase()}/${model.trim().toLowerCase()}`;
    if (!model.trim() || seen.has(key)) return;
    seen.add(key);
    out.push({ provider, model, label });
  };
  add(main.provider, main.model, `${main.provider}/${main.model}`);
  for (const p of providers) add(p.provider, p.model, p.label || `${p.provider}/${p.model}`);
  return out;
}

/** Replace (or drop, when empty) the price entry of one provider + model. */
export function upsertPrice(prices: ModelPrice[], next: ModelPrice): ModelPrice[] {
  const rest = prices.filter((p) => findModelPrice([p], next.provider, next.model) === undefined);
  return hasPrice(next) ? [...rest, next] : rest;
}

/** List prices (per million tokens) used for the status bar cost; none are built in. */
export default function ModelPricesSection({
  models,
  prices,
  onChange,
}: {
  models: PricedModel[];
  prices: ModelPrice[];
  onChange: (prices: ModelPrice[]) => void;
}) {
  const { t } = useTranslation();
  const parse = (raw: string) => {
    const n = Number(raw);
    return Number.isFinite(n) && n > 0 ? n : 0;
  };
  return (
    <section className="agentz-settings-section" data-testid="model-prices-section">
      <h3>{t("settings.modelPrices")}</h3>
      <p className="agentz-settings-hint">{t("settings.modelPricesHint")}</p>
      {models.length === 0 && <p className="agentz-settings-hint">{t("settings.modelPricesEmpty")}</p>}
      {models.map((m) => {
        const price = findModelPrice(prices, m.provider, m.model);
        const current: ModelPrice = price ?? {
          provider: m.provider,
          model: m.model,
          input_per_million: 0,
          output_per_million: 0,
          currency: "USD",
        };
        const set = (patch: Partial<ModelPrice>) =>
          onChange(upsertPrice(prices, { ...current, ...patch, provider: m.provider, model: m.model }));
        const value = (n: number) => (n > 0 ? String(n) : "");
        return (
          <div key={`${m.provider}/${m.model}`} className="agentz-model-price-row">
            <span className="agentz-model-price-name" title={`${m.provider}/${m.model}`}>
              {m.label}
            </span>
            <label>
              {t("settings.priceInput")}
              <input
                type="number"
                min={0}
                step="any"
                value={value(current.input_per_million)}
                onChange={(e) => set({ input_per_million: parse(e.target.value) })}
              />
            </label>
            <label>
              {t("settings.priceOutput")}
              <input
                type="number"
                min={0}
                step="any"
                value={value(current.output_per_million)}
                onChange={(e) => set({ output_per_million: parse(e.target.value) })}
              />
            </label>
            <select
              aria-label={t("settings.priceCurrency")}
              value={(current.currency || "USD").toUpperCase()}
              onChange={(e) => set({ currency: e.target.value })}
            >
              {CURRENCIES.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </div>
        );
      })}
    </section>
  );
}
