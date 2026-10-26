import { describe, expect, it } from "vitest";
import type { BotProfile, BotRoutine } from "../../services/tauri/botz";
import { addUsage, buildBotPrompt, extractMemoryUpdate, isRoutineDue, overBudget } from "./botRuntime";

const routine = (p: Partial<BotRoutine>): BotRoutine => ({
  id: "r",
  name: "n",
  prompt: "do it",
  kind: "interval",
  enabled: true,
  ...p,
});

const profile = (p: Partial<BotProfile>): BotProfile => ({
  id: "b",
  memory: "",
  routines: [],
  daily_token_budget: 0,
  usage_date: "",
  usage_tokens: 0,
  ...p,
});

describe("bot routines", () => {
  const now = new Date(2026, 9, 25, 10, 0, 0);

  it("interval routines fire when never run or after the interval", () => {
    expect(isRoutineDue(routine({ minutes: 30 }), now)).toBe(true);
    expect(isRoutineDue(routine({ minutes: 30, last_run_at: new Date(2026, 9, 25, 9, 45).toISOString() }), now)).toBe(false);
    expect(isRoutineDue(routine({ minutes: 30, last_run_at: new Date(2026, 9, 25, 9, 20).toISOString() }), now)).toBe(true);
    expect(isRoutineDue(routine({ enabled: false }), now)).toBe(false);
  });

  it("daily routines fire once after their time", () => {
    expect(isRoutineDue(routine({ kind: "daily", time: "09:30" }), now)).toBe(true);
    expect(isRoutineDue(routine({ kind: "daily", time: "11:00" }), now)).toBe(false);
    expect(
      isRoutineDue(routine({ kind: "daily", time: "09:30", last_run_at: new Date(2026, 9, 25, 9, 31).toISOString() }), now),
    ).toBe(false);
    expect(
      isRoutineDue(routine({ kind: "daily", time: "09:30", last_run_at: new Date(2026, 9, 24, 9, 31).toISOString() }), now),
    ).toBe(true);
    expect(isRoutineDue(routine({ kind: "daily", time: "bad" }), now)).toBe(false);
  });
});

describe("bot budget and memory", () => {
  it("resets usage on a new day and enforces the cap", () => {
    const p = addUsage(profile({ daily_token_budget: 100, usage_date: "2026-10-24", usage_tokens: 90 }), "2026-10-25", 40);
    expect(p.usage_tokens).toBe(40);
    expect(overBudget(p, "2026-10-25")).toBe(false);
    expect(overBudget(addUsage(p, "2026-10-25", 60), "2026-10-25")).toBe(true);
    expect(overBudget(profile({ usage_tokens: 1e9, usage_date: "2026-10-25" }), "2026-10-25")).toBe(false);
  });

  it("extracts memory rewrites and injects memory", () => {
    expect(extractMemoryUpdate("ok\n<memory>\nlikes tea\n</memory>")).toBe("likes tea");
    expect(extractMemoryUpdate("no block")).toBeNull();
    const prompt = buildBotPrompt("likes tea", "hello", "morning");
    expect(prompt).toContain("likes tea");
    expect(prompt).toContain("Scheduled routine: morning");
    expect(prompt.endsWith("hello")).toBe(true);
  });
});

import { budgetLevel as _budgetLevel, addUsage as _addUsage, overBudget as _overBudget } from "./botRuntime";

describe("monthly budget", () => {
  const base = {
    id: "a", memory: "", routines: [], daily_token_budget: 0, usage_date: "", usage_tokens: 0,
    monthly_token_budget: 1000, month_key: "", month_tokens: 0,
  } as unknown as Parameters<typeof _addUsage>[0];
  it("warns at 80% and pauses at 100% across days", () => {
    let p = _addUsage(base, "2026-10-26", 800);
    expect(_budgetLevel(p, "2026-10-27")).toBe("warn");
    p = _addUsage(p, "2026-10-27", 200);
    expect(_overBudget(p, "2026-10-28")).toBe(true);
    expect(_budgetLevel(p, "2026-11-01")).toBe("ok");
  });
});
