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
