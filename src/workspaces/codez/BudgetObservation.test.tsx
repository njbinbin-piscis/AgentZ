import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({ useTranslation: () => ({ i18n: { resolvedLanguage: "en" } }) }));
import BudgetObservation from "./BudgetObservation";
import type { ObserverBudgetSnapshot } from "../../services/tauri/taskDetails";
let renderer: ReactTestRenderer | undefined;
afterEach(async () => {
  await act(async () => renderer?.unmount());
  renderer = undefined;
});
const budget: ObserverBudgetSnapshot = {
  root_task_id: "task",
  policy: {
    revision: 1,
    max_total_tokens: null,
    max_active_ms: 200,
    max_cost: { currency: "USD", micro_units: 123 },
  },
  usage_count: 3,
  known_input_tokens: 17,
  known_output_tokens: 9,
  unknown_token_records: 2,
  known_active_ms: 51,
  unknown_duration_records: 1,
  known_cost_micro_units: { USD: 3, EUR: 4 },
  unknown_price_records: 2,
  observed_limits_exceeded: ["active_ms"],
};
describe("BudgetObservation", () => {
  it.each(["unknown", "missing", "stale", "unavailable", "unsupported"] as const)(
    "does not replace %s with zero totals",
    async (status) => {
      await act(async () => {
        renderer = create(<BudgetObservation source={{ status }} />);
      });
      expect(renderer!.root.findAllByType("dd")).toHaveLength(0);
      expect(JSON.stringify(renderer?.toJSON())).toContain("no remaining balance");
    },
  );
  it("separates known totals, unknown records and configured maxima", async () => {
    await act(async () => {
      renderer = create(
        <BudgetObservation source={{ status: "available", version: "revision", value: budget }} />,
      );
    });
    const values = renderer!.root.findAllByType("dd").map((node) => node.children.join(""));
    expect(values).toEqual([
      "17",
      "9",
      "51",
      "3",
      "2",
      "1",
      "2",
      "Not configured",
      "200",
      "123 USD micro-units",
    ]);
    const output = JSON.stringify(renderer?.toJSON());
    expect(output).toContain("USD");
    expect(output).toContain("EUR");
    expect(output).toContain("can be incomplete");
    expect(output).toContain("active_ms");
  });
  it("keeps available known zero distinct from absent policy and missing cost observations", async () => {
    await act(async () => {
      renderer = create(
        <BudgetObservation
          source={{
            status: "available",
            version: "v",
            value: {
              ...budget,
              known_input_tokens: 0,
              known_cost_micro_units: {},
              policy: { revision: 1, max_total_tokens: 0, max_active_ms: null, max_cost: null },
            },
          }}
        />,
      );
    });
    const values = renderer!.root.findAllByType("dd").map((node) => node.children.join(""));
    expect(values[0]).toBe("0");
    expect(values[7]).toBe("0");
    expect(values[8]).toBe("Not configured");
    expect(JSON.stringify(renderer?.toJSON())).toContain("No known cost observations");
  });
});
