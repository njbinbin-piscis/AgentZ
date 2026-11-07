import { describe, expect, it } from "vitest";
import { isChatSendResult, isTaskGateBusy, taskGateHoldSeconds } from "./taskGate";

const sample =
  "task queued/running or goal mutation active (held by src-tauri\\src\\commands\\chat.rs:395 | task key=mw6ftzle-ytktrg phase=running,4731s)";

describe("task gate errors", () => {
  it("recognises a held session gate", () => {
    expect(isTaskGateBusy(sample)).toBe(true);
    expect(isTaskGateBusy(new Error("network down"))).toBe(false);
  });

  it("accepts a finished turn payload and rejects the gate error", () => {
    expect(isChatSendResult({ ok: true, session_id: "s", response_text: "done" })).toBe(true);
    expect(isChatSendResult(new Error(sample))).toBe(false);
  });

  it("reads how long the holder has been running", () => {
    expect(taskGateHoldSeconds(sample)).toBe(4731);
    expect(taskGateHoldSeconds("task queued/running or goal mutation active (held by unknown)")).toBeNull();
  });
});
