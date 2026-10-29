import { describe, expect, it } from "vitest";
import { keepLiveToolSteps } from "./useAssistantSession";
import type { ToolTraceItem } from "../../components/ToolTrace";

const step = (id: string, status: ToolTraceItem["status"]): ToolTraceItem => ({
  id,
  name: "shell",
  input: {},
  status,
});

describe("keepLiveToolSteps", () => {
  it("pins live steps to the interrupted assistant bubble", () => {
    const out = keepLiveToolSteps(
      [{ role: "user", text: "go" }, { role: "assistant", text: "waiting" }],
      [step("a", "done"), step("b", "running")],
    );
    expect(out[1].tools?.map((t) => t.status)).toEqual(["done", "error"]);
  });

  it("keeps calls already reloaded from the database", () => {
    const reloaded = [{ role: "assistant" as const, text: "x", tools: [step("db", "done")] }];
    expect(keepLiveToolSteps(reloaded, [step("live", "done")])).toBe(reloaded);
  });

  it("does nothing without live steps", () => {
    const msgs = [{ role: "assistant" as const, text: "x" }];
    expect(keepLiveToolSteps(msgs, [])).toBe(msgs);
  });
});
