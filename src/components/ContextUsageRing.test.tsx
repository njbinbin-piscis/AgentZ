import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import ContextUsageRing, { type ContextUsageSnapshot } from "./ContextUsageRing";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string, opts?: Record<string, unknown>) => (opts ? `${key}:${JSON.stringify(opts)}` : key) }),
}));

function usage(estimated: number, budget: number): ContextUsageSnapshot {
  return {
    estimatedInputTokens: estimated,
    totalInputBudget: budget,
    triggerThreshold: Math.round(budget * 0.6),
    cumulativeInputTokens: estimated,
    cumulativeOutputTokens: 10,
    rollingSummaryVersion: 0,
    autoCompactThreshold: 0,
  };
}

function label(root: ReactTestRenderer): string {
  const node = root.root.find((n) => n.type === "span" && n.props.className === "agentz-context-ring-label");
  return String(node.children.join(""));
}

describe("ContextUsageRing percentage calculation", () => {
  let root: ReactTestRenderer | null = null;
  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("caps the label at 100% when the estimate exceeds the budget (ring and label stay in sync)", async () => {
    await act(async () => {
      root = create(<ContextUsageRing usage={usage(1230, 1000)} />);
    });
    expect(label(root!)).toBe("100%");
  });

  it("shows the true percentage when within budget", async () => {
    await act(async () => {
      root = create(<ContextUsageRing usage={usage(500, 1000)} />);
    });
    expect(label(root!)).toBe("50%");
  });

  it("keeps the uncapped estimate available in the tooltip", async () => {
    await act(async () => {
      root = create(<ContextUsageRing usage={usage(1230, 1000)} />);
    });
    const ring = root!.root.find((n) => n.type === "span" && n.props.className === "agentz-context-ring");
    expect(String(ring.props.title)).toContain("1.2k");
  });
});