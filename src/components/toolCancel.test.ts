import { describe, expect, it } from "vitest";
import { countRunTools, settledToolStatus, toolCancelKind } from "./toolCancel";

const NOT_STARTED = "Tool execution was cancelled or interrupted before completion.";

describe("toolCancelKind", () => {
  it("recognises a call that was never dispatched", () => {
    expect(toolCancelKind(NOT_STARTED, true)).toBe("skipped");
  });

  it("recognises a call that was cut short, with or without the live trace prefix", () => {
    expect(toolCancelKind("已取消", true)).toBe("interrupted");
    expect(toolCancelKind("已被用户取消", true)).toBe("interrupted");
    expect(toolCancelKind("[trace_id:9f2c] 已被用户取消", true)).toBe("interrupted");
    expect(toolCancelKind(`[trace_id:9f2c] ${NOT_STARTED}`, true)).toBe("skipped");
  });

  it("leaves real failures and successful results alone", () => {
    expect(toolCancelKind("permission denied", true)).toBeNull();
    // A tool whose own output merely mentions cancellation is a normal result.
    expect(toolCancelKind("已取消", false)).toBeNull();
    expect(toolCancelKind("订单已取消，退款处理中", true)).toBeNull();
    expect(toolCancelKind(null, true)).toBeNull();
    expect(toolCancelKind("", true)).toBeNull();
  });
});

describe("settledToolStatus", () => {
  it("maps results to the status shown in the trace", () => {
    expect(settledToolStatus("ok", false)).toEqual({ status: "done" });
    expect(settledToolStatus("boom", true)).toEqual({ status: "error" });
    expect(settledToolStatus("已被用户取消", true)).toEqual({ status: "cancelled" });
    expect(settledToolStatus(NOT_STARTED, true)).toEqual({ status: "cancelled", skipped: true });
  });
});

describe("countRunTools", () => {
  it("does not count calls that never ran", () => {
    expect(countRunTools([{}, { skipped: true }, { skipped: false }, { skipped: true }])).toBe(2);
    expect(countRunTools([])).toBe(0);
  });
});
