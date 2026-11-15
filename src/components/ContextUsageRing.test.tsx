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

function tooltip(root: ReactTestRenderer): string {
  const ring = root.root.find(
    (n) => n.type === "span" && String(n.props.className ?? "").split(" ")[0] === "agentz-context-ring",
  );
  return String(ring.props.title);
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

  it("marks an over-budget estimate as 100%+ instead of a plain 100%", async () => {
    await act(async () => {
      root = create(<ContextUsageRing usage={usage(1230, 1000)} />);
    });
    expect(label(root!)).toBe("100%+");
    // The real figure is in the tooltip.
    expect(tooltip(root!)).toContain('chat.contextRingOver:{"pct":123}');
  });

  it("shows exactly 100% without the marker when the estimate just fills the budget", async () => {
    await act(async () => {
      root = create(<ContextUsageRing usage={usage(1000, 1000)} />);
    });
    expect(label(root!)).toBe("100%");
    expect(tooltip(root!)).not.toContain("chat.contextRingOver");
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
    expect(tooltip(root!)).toContain("1.2k");
  });

  it("says nothing about compaction until one was attempted", async () => {
    await act(async () => {
      root = create(<ContextUsageRing usage={usage(500, 1000)} />);
    });
    expect(tooltip(root!)).not.toContain("chat.contextRingCompactions");
    expect(tooltip(root!)).not.toContain("chat.contextRingLastCompaction");
  });

  it("reports how many compactions ran and how the last one ended", async () => {
    await act(async () => {
      root = create(
        <ContextUsageRing
          usage={{
            ...usage(98_000, 204_000),
            lastCompaction: {
              outcome: "fallback",
              tier: "auto",
              beforeTokens: 176_205,
              afterTokens: 98_000,
              count: 2,
            },
          }}
        />,
      );
    });
    const text = tooltip(root!);
    expect(text).toContain('chat.contextRingCompactions:{"count":2}');
    expect(text).toContain(
      'chat.contextRingLastCompaction:{"status":"chat.contextCompactionFallback","before":"176.2k","after":"98.0k"}',
    );
  });
});