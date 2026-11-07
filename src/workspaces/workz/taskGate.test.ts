import { describe, expect, it } from "vitest";
import { isChatSendResult, isTaskGateBusy, sendWhenGateFree, taskGateHoldSeconds } from "./taskGate";

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

describe("sendWhenGateFree", () => {
  const fast = { delayMs: 1 };

  it("waits behind a held gate and then succeeds", async () => {
    let calls = 0;
    const result = await sendWhenGateFree(async () => {
      calls += 1;
      if (calls < 3) throw sample;
      return "ok";
    }, () => true, fast);
    expect(result).toBe("ok");
    expect(calls).toBe(3);
  });

  it("does not retry other errors", async () => {
    let calls = 0;
    await expect(
      sendWhenGateFree(async () => {
        calls += 1;
        throw new Error("network down");
      }, () => true, fast),
    ).rejects.toThrow("network down");
    expect(calls).toBe(1);
  });

  it("stops waiting once the caller says stop", async () => {
    let calls = 0;
    await expect(
      sendWhenGateFree(async () => {
        calls += 1;
        throw sample;
      }, () => false, fast),
    ).rejects.toBe(sample);
    expect(calls).toBe(1);
  });
});
